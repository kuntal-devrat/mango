//! Embedded and native system font loading, script-specific fallbacks, and decompression (WOFF/WOFF2/TTF/OTF).

use fontdue::{Font, FontSettings};
use std::borrow::Cow;

/// Embedded font data — compiled directly into the binary.
pub(crate) const FONT_SANS: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");
pub(crate) const FONT_SANS_BOLD: &[u8] = include_bytes!("../fonts/DejaVuSans-Bold.ttf");
pub(crate) const FONT_SERIF: &[u8] = include_bytes!("../fonts/DejaVuSerif.ttf");
pub(crate) const FONT_SERIF_BOLD: &[u8] = include_bytes!("../fonts/DejaVuSerif-Bold.ttf");
pub(crate) const FONT_MONO: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

pub(crate) fn load_system_font(filename: &str) -> Option<Font> {
    #[cfg(target_os = "windows")]
    {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let path = format!("{}\\Fonts\\{}", windir, filename);
        if let Ok(bytes) = std::fs::read(&path) {
            return Font::from_bytes(bytes, FontSettings::default()).ok();
        }
    }
    #[cfg(target_os = "linux")]
    {
        let selawik_name = match filename {
            "segoeui.ttf" => Some("selawk.ttf"),
            "segoeuib.ttf" => Some("selawkb.ttf"),
            _ => None,
        };
        if let Some(s_name) = selawik_name {
            let s_paths = [
                format!("/usr/share/fonts/truetype/selawik-zorin-os/{}", s_name),
                format!("/usr/share/fonts/truetype/selawik/{}", s_name),
                format!("/usr/share/fonts/truetype/{}", s_name),
            ];
            for p in s_paths {
                if let Ok(bytes) = std::fs::read(&p)
                    && let Ok(f) = Font::from_bytes(bytes, FontSettings::default())
                {
                    return Some(f);
                }
            }
        }

        let croscore_name = match filename {
            "arial.ttf" | "segoeui.ttf" => "Arimo-Regular.ttf",
            "arialbd.ttf" | "segoeuib.ttf" => "Arimo-Bold.ttf",
            "times.ttf" | "georgia.ttf" => "Tinos-Regular.ttf",
            "timesbd.ttf" | "georgiab.ttf" => "Tinos-Bold.ttf",
            "consola.ttf" | "cour.ttf" => "Cousine-Regular.ttf",
            other => other,
        };
        let liberation_name = match filename {
            "arial.ttf" | "segoeui.ttf" => "LiberationSans-Regular.ttf",
            "arialbd.ttf" | "segoeuib.ttf" => "LiberationSans-Bold.ttf",
            "times.ttf" | "georgia.ttf" => "LiberationSerif-Regular.ttf",
            "timesbd.ttf" | "georgiab.ttf" => "LiberationSerif-Bold.ttf",
            "consola.ttf" | "cour.ttf" => "LiberationMono-Regular.ttf",
            other => other,
        };
        let paths = [
            format!("/usr/share/fonts/truetype/croscore/{}", croscore_name),
            format!("/usr/share/fonts/truetype/liberation/{}", liberation_name),
            format!("/usr/share/fonts/truetype/{}", filename),
            format!("/usr/share/fonts/opentype/{}", filename),
            format!("/usr/share/fonts/TTF/{}", filename),
            format!("/usr/share/fonts/noto/{}", filename),
            format!("/usr/share/fonts/noto-cjk/{}", filename),
            format!("/usr/share/fonts/truetype/noto/{}", filename),
            format!("/usr/share/fonts/google-noto/{}", filename),
            format!("/usr/share/fonts/truetype/wqy/{}", filename),
        ];
        for p in paths {
            if let Ok(bytes) = std::fs::read(&p)
                && let Ok(f) = Font::from_bytes(bytes, FontSettings::default())
            {
                return Some(f);
            }
        }
    }
    let _ = filename;
    None
}

/// Decompresses and parses font bytes, supporting WOFF2, WOFF1, TrueType, and OpenType.
pub fn decode_font_bytes(bytes: &[u8]) -> Result<Font, String> {
    if bytes.len() < 4 {
        return Err("Font data is too short".to_string());
    }

    let trimmed = if bytes.starts_with(b" ")
        || bytes.starts_with(b"\n")
        || bytes.starts_with(b"\r")
        || bytes.starts_with(b"\t")
    {
        bytes
            .iter()
            .copied()
            .skip_while(|&b| b.is_ascii_whitespace())
            .collect::<Vec<u8>>()
    } else {
        Vec::new()
    };
    let slice = if trimmed.is_empty() { bytes } else { &trimmed };
    if slice.starts_with(b"<!DOC")
        || slice.starts_with(b"<!doc")
        || slice.starts_with(b"<html")
        || slice.starts_with(b"<HTML")
        || slice.starts_with(b"<?xml")
    {
        return Err("Expected binary font data, received HTML text".to_string());
    }

    let decompressed: Cow<[u8]> = if bytes.starts_with(b"wOF2") {
        let dec = wuff::decompress_woff2(bytes)
            .map_err(|e| format!("Failed to decompress WOFF2 font: {:?}", e))?;
        Cow::Owned(dec)
    } else if bytes.starts_with(b"wOFF") {
        let dec = wuff::decompress_woff1(bytes)
            .map_err(|e| format!("Failed to decompress WOFF1 font: {:?}", e))?;
        Cow::Owned(dec)
    } else {
        Cow::Borrowed(bytes)
    };

    Font::from_bytes(decompressed.as_ref(), FontSettings::default())
        .map_err(|e| format!("Failed to parse TrueType/OpenType font tables: {}", e))
}

/// Returns targeted system font filenames prioritized for the Unicode script of `ch`.
pub(crate) fn script_candidates_for_char(ch: char) -> &'static [&'static str] {
    let u = ch as u32;
    match u {
        // Devanagari & Indic scripts (Hindi, Bengali, Telugu, Tamil, Gujarati, Kannada, Malayalam, Gurmukhi, Oriya, Sinhala)
        0x0900..=0x0DFF => &[
            "Nirmala.ttc",
            "nirmala.ttf",
            "mangal.ttf",
            "vrinda.ttf",
            "gautami.ttf",
            "latha.ttf",
        ],
        // Arabic, Persian, Urdu
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            &[
                "NotoSansArabic-Regular.ttf",
                "tahoma.ttf",
                "tahomabd.ttf",
                "NotoKufiArabic-Regular.ttf",
                "NotoNaskhArabic-Regular.ttf",
            ]
        }
        // Hebrew
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => &[
            "NotoSansHebrew-Regular.ttf",
            "DavidLibre-Regular.ttf",
            "david.ttf",
            "tahoma.ttf",
        ],
        // Thai
        0x0E00..=0x0E7F => &[
            "LeelawUI.ttf",
            "leelawad.ttf",
            "NotoSansThai-Regular.ttf",
            "tahoma.ttf",
        ],
        // CJK Japanese (Hiragana, Katakana)
        0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF65..=0xFF9F => &[
            "msgothic.ttc",
            "meiryo.ttc",
            "YuGothR.ttc",
            "NotoSansJP-Regular.otf",
            "msyh.ttc",
            "NotoSansCJK-Regular.ttc",
        ],
        // CJK Korean (Hangul Syllables & Jamo)
        0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => &[
            "malgun.ttf",
            "malgunbd.ttf",
            "gulim.ttc",
            "NotoSansKR-Regular.otf",
            "msyh.ttc",
            "NotoSansCJK-Regular.ttc",
        ],
        // CJK Unified Ideographs (Chinese / Hanzi / Kanji / Hanja)
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF => &[
            "msyh.ttc",
            "msyhl.ttc",
            "simsun.ttc",
            "simsunb.ttf",
            "SimsunExtG.ttf",
            "msjh.ttc",
            "mingliu.ttc",
            "deng.ttf",
            "NotoSansCJK-Regular.ttc",
            "NotoSansSC-Regular.otf",
            "NotoSansTC-Regular.otf",
            "wqy-zenhei.ttc",
            "wqy-microhei.ttc",
            "msgothic.ttc",
            "malgun.ttf",
        ],
        // Armenian, Georgian, Lao
        0x0530..=0x058F | 0x10A0..=0x10FF | 0x0E80..=0x0EFF => &[
            "NotoSansArmenian-Regular.ttf",
            "NotoSansGeorgian-Regular.ttf",
            "NotoSansLao-Regular.ttf",
        ],
        // Emoji & Symbols
        0x1F000..=0x1FAFF | 0x2600..=0x27BF => &[
            "seguiemj.ttf",
            "seguisym.ttf",
            "NotoColorEmoji.ttf",
            "Symbola.ttf",
        ],
        _ => &[],
    }
}
