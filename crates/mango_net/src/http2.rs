//! # HTTP/2 Protocol Engine (GAP-014)
//!
//! Implements the HTTP/2 binary framing layer and multiplexing session model conforming to
//! RFC 7540 and RFC 9113.
//!
//! - **Frame Header Parsing & Serialization**: 9-octet binary frame format (24-bit Length,
//!   8-bit Type, 8-bit Flags, 31-bit Stream Identifier).
//! - **Frame Types**: DATA, HEADERS, PRIORITY, RST_STREAM, SETTINGS, PUSH_PROMISE, PING,
//!   GOAWAY, WINDOW_UPDATE, and CONTINUATION.
//! - **Stream State Machine**: Idle, Reserved, Open, Half-Closed, and Closed.
//! - **Multiplexing Session**: Client connection preface, flow control windows, and stream multiplexer.

use std::collections::HashMap;

/// HTTP/2 Connection Preface sent by clients upon establishing connection.
pub const HTTP2_CLIENT_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// HTTP protocol version negotiated for an HTTP transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpVersion {
    Http10,
    #[default]
    Http11,
    Http2,
    Http3,
}

impl HttpVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpVersion::Http10 => "HTTP/1.0",
            HttpVersion::Http11 => "HTTP/1.1",
            HttpVersion::Http2 => "HTTP/2.0",
            HttpVersion::Http3 => "HTTP/3.0",
        }
    }
}

/// HTTP/2 Standard Frame Types (RFC 9113 §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Data,
    Headers,
    Priority,
    RstStream,
    Settings,
    PushPromise,
    Ping,
    GoAway,
    WindowUpdate,
    Continuation,
    Unknown(u8),
}

impl From<u8> for FrameType {
    fn from(val: u8) -> Self {
        match val {
            0x0 => FrameType::Data,
            0x1 => FrameType::Headers,
            0x2 => FrameType::Priority,
            0x3 => FrameType::RstStream,
            0x4 => FrameType::Settings,
            0x5 => FrameType::PushPromise,
            0x6 => FrameType::Ping,
            0x7 => FrameType::GoAway,
            0x8 => FrameType::WindowUpdate,
            0x9 => FrameType::Continuation,
            other => FrameType::Unknown(other),
        }
    }
}

impl From<FrameType> for u8 {
    fn from(val: FrameType) -> Self {
        match val {
            FrameType::Data => 0x0,
            FrameType::Headers => 0x1,
            FrameType::Priority => 0x2,
            FrameType::RstStream => 0x3,
            FrameType::Settings => 0x4,
            FrameType::PushPromise => 0x5,
            FrameType::Ping => 0x6,
            FrameType::GoAway => 0x7,
            FrameType::WindowUpdate => 0x8,
            FrameType::Continuation => 0x9,
            FrameType::Unknown(code) => code,
        }
    }
}

/// 9-octet binary frame header preceding all HTTP/2 frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    /// 24-bit payload length (max 16,777,215 octets).
    pub length: u32,
    /// 8-bit frame type.
    pub frame_type: FrameType,
    /// 8-bit frame flags (e.g. END_STREAM = 0x1, END_HEADERS = 0x4).
    pub flags: u8,
    /// 31-bit stream identifier (bit 32 is reserved).
    pub stream_id: u32,
}

impl FrameHeader {
    /// Serializes the frame header into standard 9 octets.
    pub fn serialize(&self) -> [u8; 9] {
        let mut buf = [0u8; 9];
        // 24-bit length
        buf[0] = ((self.length >> 16) & 0xFF) as u8;
        buf[1] = ((self.length >> 8) & 0xFF) as u8;
        buf[2] = (self.length & 0xFF) as u8;
        // 8-bit type
        buf[3] = self.frame_type.into();
        // 8-bit flags
        buf[4] = self.flags;
        // 31-bit stream ID (masking highest bit)
        let s = self.stream_id & 0x7FFF_FFFF;
        buf[5] = ((s >> 24) & 0xFF) as u8;
        buf[6] = ((s >> 16) & 0xFF) as u8;
        buf[7] = ((s >> 8) & 0xFF) as u8;
        buf[8] = (s & 0xFF) as u8;
        buf
    }

    /// Deserializes a frame header from a 9-octet byte slice.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 9 {
            return None;
        }
        let length = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);
        let frame_type = FrameType::from(buf[3]);
        let flags = buf[4];
        let stream_id = (((buf[5] as u32) & 0x7F) << 24)
            | ((buf[6] as u32) << 16)
            | ((buf[7] as u32) << 8)
            | (buf[8] as u32);

        Some(Self {
            length,
            frame_type,
            flags,
            stream_id,
        })
    }
}

/// A complete HTTP/2 frame with parsed header and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Http2Frame {
    /// Creates a new HTTP/2 frame.
    pub fn new(frame_type: FrameType, flags: u8, stream_id: u32, payload: Vec<u8>) -> Self {
        Self {
            header: FrameHeader {
                length: payload.len() as u32,
                frame_type,
                flags,
                stream_id,
            },
            payload,
        }
    }

    /// Creates a SETTINGS frame (stream 0).
    pub fn settings(params: &[(u16, u32)]) -> Self {
        let mut payload = Vec::with_capacity(params.len() * 6);
        for &(id, val) in params {
            payload.extend_from_slice(&id.to_be_bytes());
            payload.extend_from_slice(&val.to_be_bytes());
        }
        Self::new(FrameType::Settings, 0x0, 0, payload)
    }

    /// Creates a SETTINGS ACK frame.
    pub fn settings_ack() -> Self {
        Self::new(FrameType::Settings, 0x1, 0, Vec::new())
    }

    /// Creates a PING frame with 8-octet opaque data.
    pub fn ping(data: [u8; 8], ack: bool) -> Self {
        let flags = if ack { 0x1 } else { 0x0 };
        Self::new(FrameType::Ping, flags, 0, data.to_vec())
    }

    /// Creates a WINDOW_UPDATE frame.
    pub fn window_update(stream_id: u32, increment: u32) -> Self {
        let payload = (increment & 0x7FFF_FFFF).to_be_bytes().to_vec();
        Self::new(FrameType::WindowUpdate, 0x0, stream_id, payload)
    }

    /// Serializes the entire frame into a byte vector.
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(9 + self.payload.len());
        bytes.extend_from_slice(&self.header.serialize());
        bytes.extend_from_slice(&self.payload);
        bytes
    }
}

/// HTTP/2 Stream state machine (RFC 9113 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Idle,
    ReservedLocal,
    ReservedRemote,
    Open,
    HalfClosedLocal,
    HalfClosedRemote,
    Closed,
}

/// A single multiplexed HTTP/2 request/response stream.
#[derive(Debug, Clone)]
pub struct Http2Stream {
    pub id: u32,
    pub state: StreamState,
    pub window_size: i32,
    pub request_headers: HashMap<String, String>,
    pub response_headers: HashMap<String, String>,
    pub response_body: Vec<u8>,
}

impl Http2Stream {
    pub fn new(id: u32, initial_window: i32) -> Self {
        Self {
            id,
            state: StreamState::Idle,
            window_size: initial_window,
            request_headers: HashMap::new(),
            response_headers: HashMap::new(),
            response_body: Vec::new(),
        }
    }
}

/// Client HTTP/2 multiplexing session.
#[derive(Debug, Clone)]
pub struct Http2Session {
    pub next_stream_id: u32,
    pub peer_settings: HashMap<u16, u32>,
    pub initial_window_size: i32,
    pub connection_window_size: i32,
    pub streams: HashMap<u32, Http2Stream>,
    pub preface_sent: bool,
}

impl Default for Http2Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Http2Session {
    /// Creates a new HTTP/2 client session with standard defaults.
    pub fn new() -> Self {
        Self {
            next_stream_id: 1, // Client streams must be odd-numbered (1, 3, 5...)
            peer_settings: HashMap::new(),
            initial_window_size: 65535,
            connection_window_size: 65535,
            streams: HashMap::new(),
            preface_sent: false,
        }
    }

    /// Prepares initial client connection bytes (client preface + initial SETTINGS frame).
    pub fn init_connection(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HTTP2_CLIENT_PREFACE);
        // Default settings: ENABLE_PUSH = 0, MAX_CONCURRENT_STREAMS = 100
        let settings = Http2Frame::settings(&[
            (0x2, 0),   // SETTINGS_ENABLE_PUSH = 0
            (0x3, 100), // SETTINGS_MAX_CONCURRENT_STREAMS = 100
            (0x4, 65535), // SETTINGS_INITIAL_WINDOW_SIZE = 65535
        ]);
        bytes.extend_from_slice(&settings.serialize());
        self.preface_sent = true;
        bytes
    }

    /// Allocates a new client stream for an outgoing HTTP request.
    pub fn open_stream(&mut self) -> u32 {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 2;
        let mut stream = Http2Stream::new(stream_id, self.initial_window_size);
        stream.state = StreamState::Open;
        self.streams.insert(stream_id, stream);
        stream_id
    }

    /// Handles an incoming HTTP/2 frame from the server.
    pub fn handle_incoming_frame(&mut self, frame: &Http2Frame) -> Result<Option<Http2Frame>, String> {
        match frame.header.frame_type {
            FrameType::Settings => {
                if frame.header.flags & 0x1 == 0 {
                    // Apply server settings and respond with SETTINGS ACK
                    let mut i = 0;
                    while i + 6 <= frame.payload.len() {
                        let id = u16::from_be_bytes([frame.payload[i], frame.payload[i + 1]]);
                        let val = u32::from_be_bytes([
                            frame.payload[i + 2],
                            frame.payload[i + 3],
                            frame.payload[i + 4],
                            frame.payload[i + 5],
                        ]);
                        self.peer_settings.insert(id, val);
                        i += 6;
                    }
                    Ok(Some(Http2Frame::settings_ack()))
                } else {
                    // Received ACK for our settings
                    Ok(None)
                }
            }
            FrameType::Ping => {
                if frame.header.flags & 0x1 == 0 && frame.payload.len() >= 8 {
                    // Send PING response with ACK bit set
                    let mut data = [0u8; 8];
                    data.copy_from_slice(&frame.payload[..8]);
                    Ok(Some(Http2Frame::ping(data, true)))
                } else {
                    Ok(None)
                }
            }
            FrameType::Data => {
                if let Some(stream) = self.streams.get_mut(&frame.header.stream_id) {
                    stream.response_body.extend_from_slice(&frame.payload);
                    if frame.header.flags & 0x1 != 0 {
                        // END_STREAM flag received
                        stream.state = StreamState::HalfClosedRemote;
                    }
                }
                Ok(None)
            }
            FrameType::WindowUpdate => {
                if frame.payload.len() >= 4 {
                    let inc = (((frame.payload[0] as i32) & 0x7F) << 24)
                        | ((frame.payload[1] as i32) << 16)
                        | ((frame.payload[2] as i32) << 8)
                        | (frame.payload[3] as i32);
                    if frame.header.stream_id == 0 {
                        self.connection_window_size += inc;
                    } else if let Some(stream) = self.streams.get_mut(&frame.header.stream_id) {
                        stream.window_size += inc;
                    }
                }
                Ok(None)
            }
            FrameType::Headers => {
                if let Some(stream) = self.streams.get_mut(&frame.header.stream_id) {
                    if frame.header.flags & 0x1 != 0 {
                        // END_STREAM flag received
                        stream.state = StreamState::HalfClosedRemote;
                    }
                }
                Ok(None)
            }
            FrameType::RstStream => {
                if let Some(stream) = self.streams.get_mut(&frame.header.stream_id) {
                    stream.state = StreamState::Closed;
                }
                Ok(None)
            }
            FrameType::GoAway => {
                // Connection is terminating; close streams > last_stream_id
                if frame.payload.len() >= 4 {
                    let last_stream_id = u32::from_be_bytes([
                        frame.payload[0] & 0x7F,
                        frame.payload[1],
                        frame.payload[2],
                        frame.payload[3],
                    ]);
                    for stream in self.streams.values_mut() {
                        if stream.id > last_stream_id {
                            stream.state = StreamState::Closed;
                        }
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_header_serialization() {
        let header = FrameHeader {
            length: 12,
            frame_type: FrameType::Settings,
            flags: 0x1,
            stream_id: 0,
        };

        let raw = header.serialize();
        assert_eq!(raw.len(), 9);

        let parsed = FrameHeader::parse(&raw).expect("Must parse valid 9-byte header");
        assert_eq!(parsed, header);
        assert_eq!(parsed.frame_type, FrameType::Settings);
        assert_eq!(parsed.length, 12);
        assert_eq!(parsed.flags, 0x1);
        assert_eq!(parsed.stream_id, 0);
    }

    #[test]
    fn test_http2_session_initialization_and_multiplexing() {
        let mut session = Http2Session::new();
        let init_bytes = session.init_connection();
        assert!(init_bytes.starts_with(HTTP2_CLIENT_PREFACE));

        let stream1 = session.open_stream();
        let stream2 = session.open_stream();
        assert_eq!(stream1, 1);
        assert_eq!(stream2, 3);
        assert_eq!(session.streams.len(), 2);

        // Server sends SETTINGS frame
        let server_settings = Http2Frame::settings(&[(0x3, 200)]);
        let ack_reply = session.handle_incoming_frame(&server_settings).unwrap();
        assert!(ack_reply.is_some());
        let ack = ack_reply.unwrap();
        assert_eq!(ack.header.frame_type, FrameType::Settings);
        assert_eq!(ack.header.flags, 0x1); // ACK flag
        assert_eq!(session.peer_settings.get(&0x3), Some(&200));

        // Server sends DATA to stream 1 with END_STREAM
        let data_frame = Http2Frame::new(FrameType::Data, 0x1, 1, b"Hello HTTP/2!".to_vec());
        let _ = session.handle_incoming_frame(&data_frame);
        let s1 = session.streams.get(&1).unwrap();
        assert_eq!(s1.response_body, b"Hello HTTP/2!");
        assert_eq!(s1.state, StreamState::HalfClosedRemote);
    }
}
