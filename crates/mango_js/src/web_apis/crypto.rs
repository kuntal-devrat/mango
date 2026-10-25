#[cfg(windows)]
pub(crate) fn fill_secure_random(buf: &mut [u8]) {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    if !buf.is_empty() {
        // SAFETY: `buf.as_mut_ptr()` is a valid, non-null, aligned pointer to `buf.len()`
        // mutable bytes allocated in memory. `SystemFunction036` (RtlGenRandom) writes
        // cryptographically secure pseudo-random bytes into the buffer up to the requested length.
        unsafe {
            SystemFunction036(buf.as_mut_ptr(), buf.len() as u32);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn fill_secure_random(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(buf);
    }
}

pub(crate) fn encode_base64_bytes(input: &[u8]) -> String {
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };

        out.push(B64_CHARS[b0 >> 2] as char);
        out.push(B64_CHARS[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((b1 & 0xF) << 2) | (b2 >> 6)] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[b2 & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}
