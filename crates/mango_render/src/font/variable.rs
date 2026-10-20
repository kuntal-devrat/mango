//! OpenType variable font `fvar` table parser and axis interpolation.

/// An OpenType variation axis extracted from the `fvar` table.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableFontAxis {
    /// 4-byte OpenType tag (e.g., `*b"wght"`).
    pub tag: [u8; 4],
    /// Human-readable axis name or tag string (e.g., "wght").
    pub tag_str: String,
    /// Minimum allowed coordinate value.
    pub min_value: f32,
    /// Default coordinate value.
    pub default_value: f32,
    /// Maximum allowed coordinate value.
    pub max_value: f32,
    /// Axis flags.
    pub flags: u16,
}

/// Metadata describing a variable font's variation space.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableFontMetadata {
    pub axes: Vec<VariableFontAxis>,
    pub instance_count: u16,
}

/// Parses the `fvar` (Font Variations) table from TrueType/OpenType font bytes.
pub fn parse_fvar_table(bytes: &[u8]) -> Option<VariableFontMetadata> {
    if bytes.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let mut fvar_offset = None;
    let mut fvar_length = 0usize;

    for i in 0..num_tables {
        let rec_start = 12 + i * 16;
        if rec_start + 16 > bytes.len() {
            break;
        }
        let tag = &bytes[rec_start..rec_start + 4];
        if tag == b"fvar" {
            let offset = u32::from_be_bytes([
                bytes[rec_start + 8],
                bytes[rec_start + 9],
                bytes[rec_start + 10],
                bytes[rec_start + 11],
            ]) as usize;
            let length = u32::from_be_bytes([
                bytes[rec_start + 12],
                bytes[rec_start + 13],
                bytes[rec_start + 14],
                bytes[rec_start + 15],
            ]) as usize;
            fvar_offset = Some(offset);
            fvar_length = length;
            break;
        }
    }

    let offset = fvar_offset?;
    if offset + 16 > bytes.len() || fvar_length < 16 {
        return None;
    }

    let axes_offset = u16::from_be_bytes([bytes[offset + 4], bytes[offset + 5]]) as usize;
    let axis_count = u16::from_be_bytes([bytes[offset + 8], bytes[offset + 9]]) as usize;
    let axis_size = u16::from_be_bytes([bytes[offset + 10], bytes[offset + 11]]) as usize;
    let instance_count = u16::from_be_bytes([bytes[offset + 12], bytes[offset + 13]]);

    if axis_size < 20 {
        return None;
    }

    let mut axes = Vec::with_capacity(axis_count);
    let mut cur = offset + axes_offset;

    for _ in 0..axis_count {
        if cur + 20 > bytes.len() {
            break;
        }
        let tag: [u8; 4] = [bytes[cur], bytes[cur + 1], bytes[cur + 2], bytes[cur + 3]];
        let min_raw = i32::from_be_bytes([
            bytes[cur + 4],
            bytes[cur + 5],
            bytes[cur + 6],
            bytes[cur + 7],
        ]);
        let def_raw = i32::from_be_bytes([
            bytes[cur + 8],
            bytes[cur + 9],
            bytes[cur + 10],
            bytes[cur + 11],
        ]);
        let max_raw = i32::from_be_bytes([
            bytes[cur + 12],
            bytes[cur + 13],
            bytes[cur + 14],
            bytes[cur + 15],
        ]);
        let flags = u16::from_be_bytes([bytes[cur + 16], bytes[cur + 17]]);

        let min_value = min_raw as f32 / 65536.0;
        let default_value = def_raw as f32 / 65536.0;
        let max_value = max_raw as f32 / 65536.0;
        let tag_str = String::from_utf8_lossy(&tag).to_string();

        axes.push(VariableFontAxis {
            tag,
            tag_str,
            min_value,
            default_value,
            max_value,
            flags,
        });

        cur += axis_size;
    }

    Some(VariableFontMetadata {
        axes,
        instance_count,
    })
}

/// Normalizes a user axis coordinate into OpenType normalized design space `[-1.0, 1.0]`.
pub fn interpolate_axis(axis: &VariableFontAxis, user_value: f32) -> f32 {
    let clamped = user_value.clamp(axis.min_value, axis.max_value);
    if (clamped - axis.default_value).abs() < 0.0001 {
        0.0
    } else if clamped < axis.default_value {
        let range = axis.default_value - axis.min_value;
        if range > 0.0001 {
            (clamped - axis.default_value) / range
        } else {
            0.0
        }
    } else {
        let range = axis.max_value - axis.default_value;
        if range > 0.0001 {
            (clamped - axis.default_value) / range
        } else {
            0.0
        }
    }
}
