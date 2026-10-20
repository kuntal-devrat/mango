//! Binary frame header parsing and serialization for Mango IPC.
//!
//! Frames follow a compact 16-octet header:
//! - Bytes 0..4: Magic bytes (`0x4D474F49` = "MGOI")
//! - Bytes 4..6: 16-bit message type identifier
//! - Bytes 6..8: 16-bit message flags (sync, response, shm reference)
//! - Bytes 8..12: 32-bit correlation identifier (for request-response matching)
//! - Bytes 12..16: 32-bit payload length

pub const IPC_MAGIC: [u8; 4] = *b"MGOI";
pub const IPC_HEADER_SIZE: usize = 16;

/// Synchronous request requiring immediate response.
pub const FLAG_SYNC: u16 = 0x0001;
/// Response frame matching a previous synchronous request.
pub const FLAG_RESPONSE: u16 = 0x0002;
/// Payload contains a shared-memory buffer reference instead of inline data.
pub const FLAG_SHM: u16 = 0x0004;

/// 16-octet binary header accompanying every IPC message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcHeader {
    pub msg_type: u16,
    pub flags: u16,
    pub correlation_id: u32,
    pub payload_length: u32,
}

impl IpcHeader {
    pub fn new(msg_type: u16, flags: u16, correlation_id: u32, payload_length: u32) -> Self {
        Self {
            msg_type,
            flags,
            correlation_id,
            payload_length,
        }
    }

    /// Serializes the 16-octet header to big-endian bytes.
    pub fn serialize(&self) -> [u8; IPC_HEADER_SIZE] {
        let mut buf = [0u8; IPC_HEADER_SIZE];
        buf[0..4].copy_from_slice(&IPC_MAGIC);
        buf[4..6].copy_from_slice(&self.msg_type.to_be_bytes());
        buf[6..8].copy_from_slice(&self.flags.to_be_bytes());
        buf[8..12].copy_from_slice(&self.correlation_id.to_be_bytes());
        buf[12..16].copy_from_slice(&self.payload_length.to_be_bytes());
        buf
    }

    /// Parses a 16-octet header from a byte buffer.
    pub fn parse(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < IPC_HEADER_SIZE {
            return Err("Buffer too short for IPC header");
        }
        if buf[0..4] != IPC_MAGIC {
            return Err("Invalid IPC magic bytes");
        }
        let msg_type = u16::from_be_bytes([buf[4], buf[5]]);
        let flags = u16::from_be_bytes([buf[6], buf[7]]);
        let correlation_id = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let payload_length = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);

        Ok(Self {
            msg_type,
            flags,
            correlation_id,
            payload_length,
        })
    }
}

/// A complete IPC frame consisting of a 16-octet header and binary payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcFrame {
    pub header: IpcHeader,
    pub payload: Vec<u8>,
}

impl IpcFrame {
    pub fn new(msg_type: u16, flags: u16, correlation_id: u32, payload: Vec<u8>) -> Self {
        let header = IpcHeader::new(msg_type, flags, correlation_id, payload.len() as u32);
        Self { header, payload }
    }

    /// Serializes header and payload into a single contiguous byte buffer.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(IPC_HEADER_SIZE + self.payload.len());
        out.extend_from_slice(&self.header.serialize());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Parses a full frame from a byte buffer.
    /// Returns the parsed frame and the number of consumed bytes.
    pub fn parse(buf: &[u8]) -> Result<Option<(Self, usize)>, &'static str> {
        if buf.len() < IPC_HEADER_SIZE {
            return Ok(None);
        }
        let header = IpcHeader::parse(&buf[..IPC_HEADER_SIZE])?;
        let total_size = IPC_HEADER_SIZE + header.payload_length as usize;
        if buf.len() < total_size {
            return Ok(None);
        }

        let payload = buf[IPC_HEADER_SIZE..total_size].to_vec();
        Ok(Some((Self { header, payload }, total_size)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_header_serialization_roundtrip() {
        let header = IpcHeader::new(42, FLAG_SYNC | FLAG_SHM, 1001, 256);
        let bytes = header.serialize();
        let parsed = IpcHeader::parse(&bytes).expect("Header parse");
        assert_eq!(parsed, header);
    }

    #[test]
    fn test_ipc_frame_roundtrip() {
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let frame = IpcFrame::new(10, FLAG_RESPONSE, 500, payload);
        let bytes = frame.serialize();
        let parsed = IpcFrame::parse(&bytes)
            .expect("Frame parse")
            .expect("Complete frame");
        assert_eq!(parsed.0, frame);
        assert_eq!(parsed.1, bytes.len());
    }
}
