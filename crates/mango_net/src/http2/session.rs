//! HTTP/2 multiplexing session, stream state machine, and connection pool.

use std::collections::HashMap;

use super::frame::{FrameType, HTTP2_CLIENT_PREFACE, Http2Frame};

/// Stream prioritization parameters (RFC 7540 §5.3 / RFC 9113).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamPriority {
    pub parent_stream_id: u32,
    pub weight: u8,
    pub exclusive: bool,
}

impl Default for StreamPriority {
    fn default() -> Self {
        Self {
            parent_stream_id: 0,
            weight: 16,
            exclusive: false,
        }
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
    pub priority: StreamPriority,
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
            priority: StreamPriority::default(),
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
    pub hpack_encoder: crate::hpack::HpackEncoder,
    pub hpack_decoder: crate::hpack::HpackDecoder,
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
            hpack_encoder: crate::hpack::HpackEncoder::new(),
            hpack_decoder: crate::hpack::HpackDecoder::new(),
            preface_sent: false,
        }
    }

    /// Prepares initial client connection bytes (client preface + initial SETTINGS frame).
    pub fn init_connection(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HTTP2_CLIENT_PREFACE);
        // Default settings: ENABLE_PUSH = 0, MAX_CONCURRENT_STREAMS = 100
        let settings = Http2Frame::settings(&[
            (0x2, 0),     // SETTINGS_ENABLE_PUSH = 0
            (0x3, 100),   // SETTINGS_MAX_CONCURRENT_STREAMS = 100
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

    /// Creates a HEADERS frame with HPACK compressed header block (RFC 7540 §6.2 / RFC 7541).
    pub fn create_headers_frame(
        &mut self,
        stream_id: u32,
        headers: &[(&str, &str)],
        end_stream: bool,
    ) -> Http2Frame {
        let fields: Vec<crate::hpack::HeaderField> = headers
            .iter()
            .map(|(k, v)| crate::hpack::HeaderField::new(*k, *v))
            .collect();
        let payload = self.hpack_encoder.encode(&fields);
        let mut flags = 0x4; // END_HEADERS
        if end_stream {
            flags |= 0x1; // END_STREAM
        }
        Http2Frame::new(FrameType::Headers, flags, stream_id, payload)
    }

    /// Decodes an HPACK compressed header block (RFC 7541).
    pub fn decode_headers_payload(
        &mut self,
        payload: &[u8],
    ) -> Result<Vec<(String, String)>, String> {
        let fields = self.hpack_decoder.decode(payload)?;
        Ok(fields.into_iter().map(|f| (f.name, f.value)).collect())
    }

    /// Handles an incoming HTTP/2 frame from the server.
    pub fn handle_incoming_frame(
        &mut self,
        frame: &Http2Frame,
    ) -> Result<Option<Http2Frame>, String> {
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
                let mut payload = &frame.payload[..];
                // RFC 7540 §6.2: Check PADDED flag (0x08)
                let pad_len = if frame.header.flags & 0x08 != 0 {
                    if payload.is_empty() {
                        return Err("Invalid HEADERS frame: missing Pad Length".to_string());
                    }
                    let p = payload[0] as usize;
                    payload = &payload[1..];
                    p
                } else {
                    0
                };

                // RFC 7540 §6.2: Check PRIORITY flag (0x20)
                let mut stream_prio = None;
                if frame.header.flags & 0x20 != 0 {
                    if payload.len() < 5 {
                        return Err("Invalid HEADERS frame: missing Priority data".to_string());
                    }
                    let dep_raw =
                        u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                    let exclusive = (dep_raw & 0x8000_0000) != 0;
                    let parent_stream_id = dep_raw & 0x7FFF_FFFF;
                    let weight = payload[4].saturating_add(1);
                    stream_prio = Some(StreamPriority {
                        parent_stream_id,
                        weight,
                        exclusive,
                    });
                    payload = &payload[5..];
                }

                // Strip trailing padding bytes
                if pad_len > 0 {
                    if payload.len() < pad_len {
                        return Err(
                            "Invalid HEADERS frame: padding exceeds payload length".to_string()
                        );
                    }
                    payload = &payload[..payload.len() - pad_len];
                }

                if let Some(stream) = self.streams.get_mut(&frame.header.stream_id) {
                    if let Some(prio) = stream_prio {
                        stream.priority = prio;
                    }
                    if let Ok(fields) = self.hpack_decoder.decode(payload) {
                        for field in fields {
                            stream.response_headers.insert(field.name, field.value);
                        }
                    }
                    if frame.header.flags & 0x1 != 0 {
                        // END_STREAM flag received
                        stream.state = StreamState::HalfClosedRemote;
                    }
                }
                Ok(None)
            }
            FrameType::Priority => {
                if frame.payload.len() >= 5 {
                    let dep_raw = u32::from_be_bytes([
                        frame.payload[0],
                        frame.payload[1],
                        frame.payload[2],
                        frame.payload[3],
                    ]);
                    let exclusive = (dep_raw & 0x8000_0000) != 0;
                    let parent_stream_id = dep_raw & 0x7FFF_FFFF;
                    let weight = frame.payload[4].saturating_add(1);
                    if let Some(stream) = self.streams.get_mut(&frame.header.stream_id) {
                        stream.priority = StreamPriority {
                            parent_stream_id,
                            weight,
                            exclusive,
                        };
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

// ---------------------------------------------------------------------------
// HTTP/2 Connection Pool
// ---------------------------------------------------------------------------

use std::sync::{Arc, Mutex};

/// Connection pool managing active HTTP/2 sessions keyed by origin ("https://example.com:443").
#[derive(Debug, Clone, Default)]
pub struct Http2ConnectionPool {
    sessions: HashMap<String, Arc<Mutex<Http2Session>>>,
}

impl Http2ConnectionPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves an existing active session for origin or creates and initializes a new one.
    pub fn get_or_create(&mut self, origin: &str) -> Arc<Mutex<Http2Session>> {
        let key = origin.trim().to_ascii_lowercase();
        self.sessions
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(Http2Session::new())))
            .clone()
    }

    /// Retrieves an existing active session if one exists.
    pub fn get_session(&self, origin: &str) -> Option<Arc<Mutex<Http2Session>>> {
        let key = origin.trim().to_ascii_lowercase();
        self.sessions.get(&key).cloned()
    }

    /// Number of active connections in the pool.
    pub fn active_connections(&self) -> usize {
        self.sessions.len()
    }

    /// Removes an origin from the pool (e.g. after GOAWAY or connection failure).
    pub fn remove(&mut self, origin: &str) {
        let key = origin.trim().to_ascii_lowercase();
        self.sessions.remove(&key);
    }

    /// Closes all sessions in the pool.
    pub fn clear(&mut self) {
        self.sessions.clear();
    }
}
