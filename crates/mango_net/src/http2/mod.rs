//! # HTTP/2 Protocol Engine
//!
//! Conforming to RFC 7540 and RFC 9113.

pub mod altsvc;
pub mod frame;
pub mod multiplex;
pub mod session;
pub mod version;

#[cfg(test)]
mod tests;

pub use altsvc::{AltSvcCache, AltSvcRecord, Http3Frame, decode_quic_varint, encode_quic_varint};
pub use frame::{FrameHeader, FrameType, HTTP2_CLIENT_PREFACE, Http2Frame};
pub use multiplex::{
    MultiplexedRequest, dispatch_multiplexed_batch, dispatch_request_on_session,
    stream_to_http_response,
};
pub use session::{
    Http2ConnectionPool, Http2Session, Http2Stream, StreamPriority, StreamState,
    SubresourcePriority,
};
pub use version::HttpVersion;
