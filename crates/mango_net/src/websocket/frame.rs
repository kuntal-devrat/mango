//! WebSocket frame definitions, encoding, and decoding (RFC 6455 §5).

use std::io::Read;

use super::client::MAX_WS_PAYLOAD;

/// WebSocket frame opcodes (RFC 6455 §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x0 => Some(Self::Continuation),
            0x1 => Some(Self::Text),
            0x2 => Some(Self::Binary),
            0x8 => Some(Self::Close),
            0x9 => Some(Self::Ping),
            0xA => Some(Self::Pong),
            _ => None,
        }
    }
}

/// Encodes an RFC 6455 client frame (always masked, per RFC 6455 §5.3).
pub fn encode_client_frame(opcode: Opcode, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut frame = Vec::new();
    // Byte 0: FIN = 1 | RSV = 0 | Opcode
    frame.push(0x80 | (opcode as u8));

    // Byte 1: Mask = 1 | Payload Len
    let len = payload.len();
    if len <= 125 {
        frame.push(0x80 | (len as u8));
    } else if len <= 65535 {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    // Mask key (4 bytes)
    frame.extend_from_slice(&mask);

    // Masked payload
    for (i, &byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[i % 4]);
    }

    frame
}

/// Encodes an RFC 6455 server frame (unmasked, per RFC 6455 §5.1).
pub fn encode_server_frame(opcode: Opcode, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    // Byte 0: FIN = 1 | RSV = 0 | Opcode
    frame.push(0x80 | (opcode as u8));

    // Byte 1: Mask = 0 | Payload Len
    let len = payload.len();
    if len <= 125 {
        frame.push(len as u8);
    } else if len <= 65535 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    // Unmasked payload
    frame.extend_from_slice(payload);
    frame
}

/// Decodes a single RFC 6455 frame from a reader.
///
/// Enforces: RSV bits must be 0, minimal length encoding, control frames
/// (`Close`/`Ping`/`Pong`) must have payload ≤125 and FIN=1, payload capped
/// at [`MAX_WS_PAYLOAD`], and `Text` payloads must be valid UTF-8.
pub fn decode_frame<R: Read>(reader: &mut R) -> std::io::Result<(Opcode, Vec<u8>, bool)> {
    let mut header = [0u8; 2];
    reader.read_exact(&mut header)?;

    let fin = (header[0] & 0x80) != 0;
    // RSV1/RSV2/RSV3 must be 0 (no extensions negotiated).
    if (header[0] & 0x70) != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "RSV bits must be 0 without negotiated extensions",
        ));
    }
    let opcode_byte = header[0] & 0x0F;
    let opcode = Opcode::from_u8(opcode_byte)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown opcode"))?;

    let masked = (header[1] & 0x80) != 0;
    let len_code = header[1] & 0x7F;

    let payload_len: usize = if len_code <= 125 {
        len_code as usize
    } else if len_code == 126 {
        let mut ext = [0u8; 2];
        reader.read_exact(&mut ext)?;
        let v = u16::from_be_bytes(ext) as usize;
        // Minimal-length encoding: 126 must mean >125.
        if v <= 125 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Non-minimal frame length encoding (126 for len <= 125)",
            ));
        }
        v
    } else {
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext)?;
        // MSB must be 0 per RFC 6455 §5.2.
        if (ext[0] & 0x80) != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Frame length MSB must be 0",
            ));
        }
        let v = u64::from_be_bytes(ext);
        if v <= 65535 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Non-minimal frame length encoding (127 for len <= 65535)",
            ));
        }
        if v > MAX_WS_PAYLOAD as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WebSocket frame exceeds 16MiB cap",
            ));
        }
        v as usize
    };

    // Control frames must be ≤125 bytes and unfragmented.
    if matches!(opcode, Opcode::Close | Opcode::Ping | Opcode::Pong) {
        if !fin {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Fragmented control frame",
            ));
        }
        if payload_len > 125 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Control frame payload exceeds 125 bytes",
            ));
        }
    }

    if payload_len > MAX_WS_PAYLOAD {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WebSocket frame exceeds 16MiB cap",
        ));
    }

    let mask_key = if masked {
        let mut m = [0u8; 4];
        reader.read_exact(&mut m)?;
        Some(m)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload)?;

    if let Some(mask) = mask_key {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
    }

    // Text frames must be valid UTF-8; fail the connection otherwise.
    if opcode == Opcode::Text && std::str::from_utf8(&payload).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid UTF-8 in text frame",
        ));
    }
    // Close reason (bytes 2..) must be valid UTF-8 per RFC 6455 §5.5.1.
    if opcode == Opcode::Close && payload.len() > 2 && std::str::from_utf8(&payload[2..]).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid UTF-8 in close reason",
        ));
    }

    Ok((opcode, payload, fin))
}
