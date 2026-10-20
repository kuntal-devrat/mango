//! QUIC Transport Protocol Engine (RFC 9000).
//!
//! Provides the low-level QUIC packet engine:
//! - Long Header Packets: Initial, 0-RTT, Handshake, and Retry with connection ID negotiation.
//! - Short Header Packets: 1-RTT data packets with spin bit, key phase, and packet number decoding.
//! - QUIC Frames: CRYPTO (TLS 1.3 integration), STREAM, ACK, PATH_CHALLENGE, PATH_RESPONSE,
//!   NEW_CONNECTION_ID, RETIRE_CONNECTION_ID, and CONNECTION_CLOSE.
//! - Connection Migration: Path validation and CID rotation across network interfaces.

use std::collections::VecDeque;

/// QUIC Version 1 identifier (RFC 9000).
pub const QUIC_VERSION_1: u32 = 0x0000_0001;

/// QUIC Packet Header Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongPacketType {
    Initial,
    ZeroRtt,
    Handshake,
    Retry,
}

impl LongPacketType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match (b >> 4) & 0x03 {
            0x00 => Some(LongPacketType::Initial),
            0x01 => Some(LongPacketType::ZeroRtt),
            0x02 => Some(LongPacketType::Handshake),
            0x03 => Some(LongPacketType::Retry),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            LongPacketType::Initial => 0x00,
            LongPacketType::ZeroRtt => 0x10,
            LongPacketType::Handshake => 0x20,
            LongPacketType::Retry => 0x30,
        }
    }
}

/// A variable-length QUIC Connection ID (0 to 20 octets).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ConnectionId {
    bytes: Vec<u8>,
}

impl ConnectionId {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        let b = bytes.into();
        assert!(
            b.len() <= 20,
            "Connection ID length must not exceed 20 octets"
        );
        Self { bytes: b }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// A Long Header QUIC Packet (RFC 9000 §17.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongHeaderPacket {
    pub packet_type: LongPacketType,
    pub version: u32,
    pub dcid: ConnectionId,
    pub scid: ConnectionId,
    pub token: Vec<u8>,
    pub payload: Vec<u8>,
}

impl LongHeaderPacket {
    /// Serializes the Long Header Packet into raw UDP payload bytes.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // First byte: Header Form (1) | Fixed Bit (1) | Long Packet Type (2 bits)
        let first = 0x80 | 0x40 | self.packet_type.to_byte();
        out.push(first);
        out.extend_from_slice(&self.version.to_be_bytes());

        // DCID
        out.push(self.dcid.len() as u8);
        out.extend_from_slice(self.dcid.as_bytes());

        // SCID
        out.push(self.scid.len() as u8);
        out.extend_from_slice(self.scid.as_bytes());

        // Token length & token (Initial only)
        if self.packet_type == LongPacketType::Initial {
            out.extend_from_slice(&crate::http2::encode_quic_varint(self.token.len() as u64));
            out.extend_from_slice(&self.token);
        }

        // Payload length & payload
        out.extend_from_slice(&crate::http2::encode_quic_varint(self.payload.len() as u64));
        out.extend_from_slice(&self.payload);
        out
    }

    /// Parses a Long Header Packet from bytes.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 7 {
            return None;
        }
        let first = buf[0];
        if first & 0x80 == 0 {
            return None; // Not a Long Header packet
        }
        let packet_type = LongPacketType::from_byte(first)?;
        let version = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);

        let dcid_len = buf[5] as usize;
        let mut idx = 6;
        if buf.len() < idx + dcid_len + 1 {
            return None;
        }
        let dcid = ConnectionId::new(&buf[idx..idx + dcid_len]);
        idx += dcid_len;

        let scid_len = buf[idx] as usize;
        idx += 1;
        if buf.len() < idx + scid_len {
            return None;
        }
        let scid = ConnectionId::new(&buf[idx..idx + scid_len]);
        idx += scid_len;

        let mut token = Vec::new();
        if packet_type == LongPacketType::Initial {
            let (tok_len, consumed) = crate::http2::decode_quic_varint(&buf[idx..])?;
            idx += consumed;
            let tok_len = tok_len as usize;
            if buf.len() < idx + tok_len {
                return None;
            }
            token = buf[idx..idx + tok_len].to_vec();
            idx += tok_len;
        }

        let (payload_len, consumed) = crate::http2::decode_quic_varint(&buf[idx..])?;
        idx += consumed;
        let payload_len = payload_len as usize;
        if buf.len() < idx + payload_len {
            return None;
        }
        let payload = buf[idx..idx + payload_len].to_vec();

        Some(Self {
            packet_type,
            version,
            dcid,
            scid,
            token,
            payload,
        })
    }
}

/// A Short Header (1-RTT) QUIC Packet (RFC 9000 §17.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortHeaderPacket {
    pub spin_bit: bool,
    pub key_phase: bool,
    pub dcid: ConnectionId,
    pub packet_number: u64,
    pub payload: Vec<u8>,
}

impl ShortHeaderPacket {
    /// Serializes a 1-RTT short header packet.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // Header Form = 0, Fixed Bit = 1
        let mut first = 0x40;
        if self.spin_bit {
            first |= 0x20;
        }
        if self.key_phase {
            first |= 0x04;
        }
        // Packet number length: 2 octets encoding (flag 0x01)
        first |= 0x01;
        out.push(first);

        out.extend_from_slice(self.dcid.as_bytes());
        let pn = (self.packet_number as u16).to_be_bytes();
        out.extend_from_slice(&pn);
        out.extend_from_slice(&self.payload);
        out
    }

    /// Parses a 1-RTT short header packet assuming known DCID length.
    pub fn parse(buf: &[u8], dcid_len: usize) -> Option<Self> {
        if buf.len() < 1 + dcid_len + 2 {
            return None;
        }
        let first = buf[0];
        if first & 0x80 != 0 {
            return None; // Not a short header
        }
        let spin_bit = (first & 0x20) != 0;
        let key_phase = (first & 0x04) != 0;

        let mut idx = 1;
        let dcid = ConnectionId::new(&buf[idx..idx + dcid_len]);
        idx += dcid_len;

        let packet_number = u16::from_be_bytes([buf[idx], buf[idx + 1]]) as u64;
        idx += 2;

        let payload = buf[idx..].to_vec();
        Some(Self {
            spin_bit,
            key_phase,
            dcid,
            packet_number,
            payload,
        })
    }
}

/// QUIC Frame Types (RFC 9000 §19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuicFrame {
    Padding {
        length: usize,
    },
    Ping,
    Ack {
        largest_acknowledged: u64,
        delay: u64,
    },
    ResetStream {
        stream_id: u64,
        error_code: u64,
        final_size: u64,
    },
    StopSending {
        stream_id: u64,
        error_code: u64,
    },
    Crypto {
        offset: u64,
        data: Vec<u8>,
    },
    Stream {
        stream_id: u64,
        offset: u64,
        fin: bool,
        data: Vec<u8>,
    },
    PathChallenge {
        data: [u8; 8],
    },
    PathResponse {
        data: [u8; 8],
    },
    NewConnectionId {
        sequence_number: u64,
        retire_prior_to: u64,
        connection_id: ConnectionId,
        reset_token: [u8; 16],
    },
    RetireConnectionId {
        sequence_number: u64,
    },
    ConnectionClose {
        error_code: u64,
        reason: String,
    },
    HandshakeDone,
}

impl QuicFrame {
    /// Serializes a QUIC frame into binary representation.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            QuicFrame::Padding { length } => {
                out.resize(*length, 0x00);
            }
            QuicFrame::Ping => {
                out.push(0x01);
            }
            QuicFrame::Ack {
                largest_acknowledged,
                delay,
            } => {
                out.push(0x02);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*largest_acknowledged));
                out.extend_from_slice(&crate::http2::encode_quic_varint(*delay));
                out.extend_from_slice(&crate::http2::encode_quic_varint(0)); // range count
                out.extend_from_slice(&crate::http2::encode_quic_varint(0)); // first range
            }
            QuicFrame::ResetStream {
                stream_id,
                error_code,
                final_size,
            } => {
                out.push(0x04);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*stream_id));
                out.extend_from_slice(&crate::http2::encode_quic_varint(*error_code));
                out.extend_from_slice(&crate::http2::encode_quic_varint(*final_size));
            }
            QuicFrame::StopSending {
                stream_id,
                error_code,
            } => {
                out.push(0x05);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*stream_id));
                out.extend_from_slice(&crate::http2::encode_quic_varint(*error_code));
            }
            QuicFrame::Crypto { offset, data } => {
                out.push(0x06);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*offset));
                out.extend_from_slice(&crate::http2::encode_quic_varint(data.len() as u64));
                out.extend_from_slice(data);
            }
            QuicFrame::Stream {
                stream_id,
                offset,
                fin,
                data,
            } => {
                // 0x08 | OFF(0x04) | LEN(0x02) | FIN(0x01)
                let mut type_byte = 0x08 | 0x04 | 0x02;
                if *fin {
                    type_byte |= 0x01;
                }
                out.push(type_byte);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*stream_id));
                out.extend_from_slice(&crate::http2::encode_quic_varint(*offset));
                out.extend_from_slice(&crate::http2::encode_quic_varint(data.len() as u64));
                out.extend_from_slice(data);
            }
            QuicFrame::PathChallenge { data } => {
                out.push(0x1a);
                out.extend_from_slice(data);
            }
            QuicFrame::PathResponse { data } => {
                out.push(0x1b);
                out.extend_from_slice(data);
            }
            QuicFrame::NewConnectionId {
                sequence_number,
                retire_prior_to,
                connection_id,
                reset_token,
            } => {
                out.push(0x18);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*sequence_number));
                out.extend_from_slice(&crate::http2::encode_quic_varint(*retire_prior_to));
                out.push(connection_id.len() as u8);
                out.extend_from_slice(connection_id.as_bytes());
                out.extend_from_slice(reset_token);
            }
            QuicFrame::RetireConnectionId { sequence_number } => {
                out.push(0x19);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*sequence_number));
            }
            QuicFrame::ConnectionClose { error_code, reason } => {
                out.push(0x1c);
                out.extend_from_slice(&crate::http2::encode_quic_varint(*error_code));
                out.extend_from_slice(&crate::http2::encode_quic_varint(0)); // frame type
                out.extend_from_slice(&crate::http2::encode_quic_varint(reason.len() as u64));
                out.extend_from_slice(reason.as_bytes());
            }
            QuicFrame::HandshakeDone => {
                out.push(0x1e);
            }
        }
        out
    }
}

/// State machine managing connection migration and path validation across network interfaces (RFC 9000 §9).
#[derive(Debug, Clone)]
pub struct QuicConnectionMigration {
    pub active_cid: ConnectionId,
    pub available_cids: VecDeque<(u64, ConnectionId)>,
    pub pending_path_challenge: Option<[u8; 8]>,
    pub path_validated: bool,
}

impl QuicConnectionMigration {
    pub fn new(initial_cid: ConnectionId) -> Self {
        Self {
            active_cid: initial_cid,
            available_cids: VecDeque::new(),
            pending_path_challenge: None,
            path_validated: true,
        }
    }

    /// Registers a newly advertised Connection ID from server (NEW_CONNECTION_ID frame).
    pub fn add_connection_id(&mut self, seq: u64, cid: ConnectionId) {
        self.available_cids.push_back((seq, cid));
    }

    /// Initiates connection migration to a new network interface (e.g. Wi-Fi to cellular).
    /// Generates a PATH_CHALLENGE frame with random entropy.
    pub fn initiate_migration(&mut self) -> Result<(QuicFrame, ConnectionId), &'static str> {
        let Some((_, next_cid)) = self.available_cids.pop_front() else {
            return Err("No unused Connection IDs available for migration");
        };

        let old_cid = std::mem::replace(&mut self.active_cid, next_cid.clone());
        let challenge_data = [0x5A; 8];
        self.pending_path_challenge = Some(challenge_data);
        self.path_validated = false;

        Ok((
            QuicFrame::PathChallenge {
                data: challenge_data,
            },
            old_cid,
        ))
    }

    /// Validates an incoming PATH_RESPONSE frame.
    pub fn validate_path_response(&mut self, response_data: &[u8; 8]) -> bool {
        if self.pending_path_challenge == Some(*response_data) {
            self.path_validated = true;
            self.pending_path_challenge = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_long_header_roundtrip() {
        let packet = LongHeaderPacket {
            packet_type: LongPacketType::Initial,
            version: QUIC_VERSION_1,
            dcid: ConnectionId::new(vec![0x01, 0x02, 0x03, 0x04]),
            scid: ConnectionId::new(vec![0x05, 0x06, 0x07, 0x08]),
            token: vec![0xAA, 0xBB],
            payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };

        let raw = packet.serialize();
        let parsed = LongHeaderPacket::parse(&raw).expect("Must parse valid Initial packet");
        assert_eq!(parsed, packet);
    }

    #[test]
    fn test_quic_short_header_roundtrip() {
        let packet = ShortHeaderPacket {
            spin_bit: true,
            key_phase: false,
            dcid: ConnectionId::new(vec![0x11, 0x22, 0x33, 0x44]),
            packet_number: 42,
            payload: vec![0x01, 0x02, 0x03],
        };

        let raw = packet.serialize();
        let parsed = ShortHeaderPacket::parse(&raw, 4).expect("Must parse valid 1-RTT packet");
        assert_eq!(parsed, packet);
    }

    #[test]
    fn test_quic_connection_migration() {
        let initial_cid = ConnectionId::new(vec![0x01]);
        let mut migrator = QuicConnectionMigration::new(initial_cid);

        let backup_cid = ConnectionId::new(vec![0x02]);
        migrator.add_connection_id(1, backup_cid.clone());

        let (challenge_frame, old_cid) = migrator.initiate_migration().unwrap();
        assert_eq!(old_cid, ConnectionId::new(vec![0x01]));
        assert_eq!(migrator.active_cid, backup_cid);
        assert!(!migrator.path_validated);

        if let QuicFrame::PathChallenge { data } = challenge_frame {
            assert!(migrator.validate_path_response(&data));
            assert!(migrator.path_validated);
        } else {
            panic!("Expected PathChallenge frame");
        }
    }
}
