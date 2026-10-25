//! HTTP/2 multiplexing session, stream state machine, and connection pool.

use std::collections::HashMap;

use super::frame::{FrameType, HTTP2_CLIENT_PREFACE, Http2Frame};

/// Classification of subresource priority for HTTP/2 stream scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubresourcePriority {
    /// Render-blocking stylesheets (Highest priority: weight = 255).
    Style,
    /// Parser-blocking and deferred JavaScript (High priority: weight = 219).
    Script,
    /// Web fonts required for text layout (Medium priority: weight = 179).
    Font,
    /// Document HTML / navigation (Highest priority: weight = 255).
    Document,
    /// Visible or off-screen images (Low priority: weight = 99).
    Image,
    /// Background media or lowest priority items (Lowest priority: weight = 15).
    Other,
}

/// Stream prioritization parameters (RFC 7540 §5.3 / RFC 9113).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamPriority {
    pub parent_stream_id: u32,
    pub weight: u8,
    pub exclusive: bool,
}

impl StreamPriority {
    /// Priority for render-blocking stylesheets and main document (weight = 255).
    pub const STYLE: Self = Self {
        parent_stream_id: 0,
        weight: 255,
        exclusive: false,
    };

    /// Priority for scripts (weight = 219).
    pub const SCRIPT: Self = Self {
        parent_stream_id: 0,
        weight: 219,
        exclusive: false,
    };

    /// Priority for web fonts (weight = 179).
    pub const FONT: Self = Self {
        parent_stream_id: 0,
        weight: 179,
        exclusive: false,
    };

    /// Priority for images and media (weight = 99).
    pub const IMAGE: Self = Self {
        parent_stream_id: 0,
        weight: 99,
        exclusive: false,
    };

    /// Low / default priority (weight = 15).
    pub const LOW: Self = Self {
        parent_stream_id: 0,
        weight: 15,
        exclusive: false,
    };

    /// Returns standard stream priority parameters for a given subresource type.
    pub fn for_resource_type(resource_type: SubresourcePriority) -> Self {
        match resource_type {
            SubresourcePriority::Document | SubresourcePriority::Style => Self::STYLE,
            SubresourcePriority::Script => Self::SCRIPT,
            SubresourcePriority::Font => Self::FONT,
            SubresourcePriority::Image => Self::IMAGE,
            SubresourcePriority::Other => Self::LOW,
        }
    }
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
    pub outbound_queue: Vec<Http2Frame>,
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
            outbound_queue: Vec::new(),
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

    /// Allocates a new client stream for an outgoing HTTP request with default priority.
    pub fn open_stream(&mut self) -> u32 {
        self.open_stream_with_priority(StreamPriority::default())
    }

    /// Allocates a new client stream with explicit stream prioritization.
    pub fn open_stream_with_priority(&mut self, priority: StreamPriority) -> u32 {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 2;
        let mut stream = Http2Stream::new(stream_id, self.initial_window_size);
        stream.state = StreamState::Open;
        stream.priority = priority;
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
        let priority = self
            .streams
            .get(&stream_id)
            .map(|s| s.priority)
            .unwrap_or_default();
        self.create_headers_frame_with_priority(stream_id, headers, end_stream, priority)
    }

    /// Creates a HEADERS frame with embedded RFC 7540 §6.2 stream priority fields (PRIORITY flag 0x20).
    pub fn create_headers_frame_with_priority(
        &mut self,
        stream_id: u32,
        headers: &[(&str, &str)],
        end_stream: bool,
        priority: StreamPriority,
    ) -> Http2Frame {
        if let Some(stream) = self.streams.get_mut(&stream_id) {
            stream.priority = priority;
        }
        let fields: Vec<crate::hpack::HeaderField> = headers
            .iter()
            .map(|(k, v)| crate::hpack::HeaderField::new(*k, *v))
            .collect();
        let encoded_headers = self.hpack_encoder.encode(&fields);

        // Prepend 5-byte Priority specification (RFC 7540 §6.2)
        let dep_raw =
            ((priority.exclusive as u32) << 31) | (priority.parent_stream_id & 0x7FFF_FFFF);
        let mut payload = Vec::with_capacity(5 + encoded_headers.len());
        payload.extend_from_slice(&dep_raw.to_be_bytes());
        payload.push(priority.weight);
        payload.extend_from_slice(&encoded_headers);

        let mut flags = 0x4 | 0x20; // END_HEADERS | PRIORITY
        if end_stream {
            flags |= 0x1; // END_STREAM
        }
        Http2Frame::new(FrameType::Headers, flags, stream_id, payload)
    }

    /// Creates an RFC 7540 §6.3 PRIORITY frame for an active stream.
    pub fn create_priority_frame(
        &mut self,
        stream_id: u32,
        priority: StreamPriority,
    ) -> Http2Frame {
        if let Some(stream) = self.streams.get_mut(&stream_id) {
            stream.priority = priority;
        }
        let dep_raw =
            ((priority.exclusive as u32) << 31) | (priority.parent_stream_id & 0x7FFF_FFFF);
        let mut payload = Vec::with_capacity(5);
        payload.extend_from_slice(&dep_raw.to_be_bytes());
        payload.push(priority.weight);
        Http2Frame::new(FrameType::Priority, 0x0, stream_id, payload)
    }

    /// Enqueues an outgoing frame awaiting multiplexed transmission.
    pub fn enqueue_frame(&mut self, frame: Http2Frame) {
        self.outbound_queue.push(frame);
    }

    /// Number of queued frames awaiting transmission.
    pub fn queued_frame_count(&self) -> usize {
        self.outbound_queue.len()
    }

    /// Opens a new stream with the given priority and enqueues its initial HEADERS request frame.
    pub fn send_request_with_priority(
        &mut self,
        headers: &[(&str, &str)],
        end_stream: bool,
        priority: StreamPriority,
    ) -> u32 {
        let stream_id = self.open_stream_with_priority(priority);
        let frame =
            self.create_headers_frame_with_priority(stream_id, headers, end_stream, priority);
        self.enqueue_frame(frame);
        stream_id
    }

    /// Enqueues a DATA frame on an existing stream.
    pub fn send_data(&mut self, stream_id: u32, data: Vec<u8>, end_stream: bool) {
        let flags = if end_stream { 0x1 } else { 0x0 };
        let frame = Http2Frame::new(FrameType::Data, flags, stream_id, data);
        self.enqueue_frame(frame);
    }

    /// Schedules and drains outbound frames according to stream priority weights and dependency order.
    ///
    /// Frames on higher-priority streams (e.g. CSS weight 255 and JS weight 219)
    /// are scheduled before lower-priority streams (e.g. fonts weight 179 and images weight 99).
    /// Control frames (stream 0) always take absolute precedence over all stream frames.
    pub fn schedule_outbound_frames(&mut self) -> Vec<Http2Frame> {
        let mut queued: Vec<(usize, u16, Http2Frame)> = self
            .outbound_queue
            .drain(..)
            .enumerate()
            .map(|(seq, frame)| {
                let sid = frame.header.stream_id;
                let weight: u16 = if sid == 0 {
                    u16::MAX // Stream 0 control frames always take absolute precedence
                } else {
                    self.streams
                        .get(&sid)
                        .map(|s| s.priority.weight as u16)
                        .unwrap_or(16)
                };
                (seq, weight, frame)
            })
            .collect();

        // Sort descending by weight, preserving arrival sequence (seq) for ties
        queued.sort_by(|a, b| {
            b.1.cmp(&a.1) // Higher weight first
                .then_with(|| a.0.cmp(&b.0)) // Earlier arrival first
        });

        queued.into_iter().map(|(_, _, frame)| frame).collect()
    }

    /// Pops the single highest-priority scheduled frame from the outbound queue.
    pub fn pop_next_scheduled_frame(&mut self) -> Option<Http2Frame> {
        if self.outbound_queue.is_empty() {
            return None;
        }
        let scheduled = self.schedule_outbound_frames();
        let mut iter = scheduled.into_iter();
        let next = iter.next();
        self.outbound_queue = iter.collect();
        next
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
