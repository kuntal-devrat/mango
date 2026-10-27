//! Global font manager singleton, face selection, family resolution, and text measurement.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use fontdue::{Font, FontSettings};

use super::emoji::is_emoji_char;
use super::loader::{
    FONT_AHEM, FONT_MONO, FONT_SANS, FONT_SANS_BOLD, FONT_SERIF, FONT_SERIF_BOLD,
    decode_font_bytes, load_system_font, script_candidates_for_char,
};
use super::types::{
    FONT_ACCESS_TICK, FontFamily, FontWeight, MAX_WEB_FONT_FAMILIES, WebFontEntry,
    bump_web_font_epoch, web_font_epoch,
};

/// Global singleton for the font manager.
static FONT_MANAGER: OnceLock<FontManager> = OnceLock::new();

/// Returns a reference to the global [`FontManager`] singleton.
///
/// Initializes on first call by parsing the embedded TTF font data.
pub fn font_manager() -> &'static FontManager {
    FONT_MANAGER.get_or_init(FontManager::new)
}

/// Prunes custom web fonts loaded across navigations, reclaiming heap allocations.
pub fn prune_web_fonts() {
    font_manager().prune_web_fonts();
}

/// Manages loaded font faces and provides text measurement and glyph rasterization.
pub struct FontManager {
    pub(crate) sans_regular: Arc<Font>,
    pub(crate) sans_bold: Arc<Font>,
    pub(crate) serif_regular: Arc<Font>,
    pub(crate) serif_bold: Arc<Font>,
    pub(crate) mono: Arc<Font>,
    pub(crate) fallback_fonts: RwLock<Vec<Arc<Font>>>,
    pub(crate) unloaded_fallbacks: RwLock<Vec<&'static str>>,
    pub(crate) missing_glyphs: RwLock<HashSet<char>>,
    pub(crate) web_fonts: RwLock<HashMap<u32, WebFontEntry>>,
    pub(crate) family_to_id: RwLock<HashMap<String, u32>>,
    pub(crate) next_font_id: AtomicU32,
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FontManager {
    /// Creates a new `FontManager` prioritizing native platform system fonts,
    /// with graceful fallback to embedded DejaVu fonts.
    pub fn new() -> Self {
        let settings = FontSettings::default();

        let sans_regular = Arc::new(
            load_system_font("arial.ttf")
                .or_else(|| load_system_font("segoeui.ttf"))
                .unwrap_or_else(|| {
                    Font::from_bytes(FONT_SANS, settings)
                        .expect("Failed to parse embedded DejaVuSans.ttf")
                }),
        );
        let sans_bold = Arc::new(
            load_system_font("arialbd.ttf")
                .or_else(|| load_system_font("segoeuib.ttf"))
                .unwrap_or_else(|| {
                    Font::from_bytes(FONT_SANS_BOLD, settings)
                        .expect("Failed to parse embedded DejaVuSans-Bold.ttf")
                }),
        );
        let serif_regular = Arc::new(
            load_system_font("times.ttf")
                .or_else(|| load_system_font("georgia.ttf"))
                .unwrap_or_else(|| {
                    Font::from_bytes(FONT_SERIF, settings)
                        .expect("Failed to parse embedded DejaVuSerif.ttf")
                }),
        );
        let serif_bold = Arc::new(
            load_system_font("timesbd.ttf")
                .or_else(|| load_system_font("georgiab.ttf"))
                .unwrap_or_else(|| {
                    Font::from_bytes(FONT_SERIF_BOLD, settings)
                        .expect("Failed to parse embedded DejaVuSerif-Bold.ttf")
                }),
        );
        let mono = Arc::new(
            load_system_font("consola.ttf")
                .or_else(|| load_system_font("cour.ttf"))
                .unwrap_or_else(|| {
                    Font::from_bytes(FONT_MONO, settings)
                        .expect("Failed to parse embedded DejaVuSansMono.ttf")
                }),
        );

        let fallback_names: Vec<&'static str> = vec![
            "Nirmala.ttc",
            "nirmala.ttf",
            "mangal.ttf",
            "vrinda.ttf",
            "gautami.ttf",
            "latha.ttf",
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
            "meiryo.ttc",
            "YuGothR.ttc",
            "NotoSansJP-Regular.otf",
            "malgun.ttf",
            "malgunbd.ttf",
            "gulim.ttc",
            "NotoSansKR-Regular.otf",
            "NotoSansArabic-Regular.ttf",
            "NotoKufiArabic-Regular.ttf",
            "NotoNaskhArabic-Regular.ttf",
            "tahoma.ttf",
            "tahomabd.ttf",
            "LeelawUI.ttf",
            "leelawad.ttf",
            "NotoSansThai-Regular.ttf",
            "NotoSansHebrew-Regular.ttf",
            "DavidLibre-Regular.ttf",
            "DavidCLM-Medium.otf",
            "david.ttf",
            "NotoSansArmenian-Regular.ttf",
            "NotoSansGeorgian-Regular.ttf",
            "NotoSansLao-Regular.ttf",
            "seguiemj.ttf",
            "seguisym.ttf",
            "NotoColorEmoji.ttf",
            "Symbola.ttf",
        ];

        let mgr = Self {
            sans_regular,
            sans_bold,
            serif_regular,
            serif_bold,
            mono,
            fallback_fonts: RwLock::new(Vec::new()),
            unloaded_fallbacks: RwLock::new(fallback_names),
            missing_glyphs: RwLock::new(HashSet::new()),
            web_fonts: RwLock::new(HashMap::new()),
            family_to_id: RwLock::new(HashMap::new()),
            next_font_id: AtomicU32::new(1),
        };
        let _ = mgr.register_web_font("Ahem", FontWeight::Regular, FONT_AHEM);
        let _ = mgr.register_web_font("Ahem", FontWeight::Bold, FONT_AHEM);
        mgr
    }

    /// Selects the appropriate font face based on family and weight.
    pub fn select_font(&self, family: FontFamily, weight: FontWeight) -> Arc<Font> {
        match family {
            FontFamily::Monospace => Arc::clone(&self.mono),
            FontFamily::Serif => match weight {
                FontWeight::Bold => Arc::clone(&self.serif_bold),
                FontWeight::Regular => Arc::clone(&self.serif_regular),
            },
            FontFamily::SansSerif => match weight {
                FontWeight::Bold => Arc::clone(&self.sans_bold),
                FontWeight::Regular => Arc::clone(&self.sans_regular),
            },
            FontFamily::Custom(id) => {
                if let Ok(web_fonts) = self.web_fonts.read()
                    && let Some(entry) = web_fonts.get(&id)
                {
                    entry.last_accessed.store(
                        FONT_ACCESS_TICK.fetch_add(1, Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    let font_opt = match weight {
                        FontWeight::Bold => entry.bold.first().or_else(|| entry.regular.first()),
                        FontWeight::Regular => entry.regular.first().or_else(|| entry.bold.first()),
                    };
                    if let Some(font) = font_opt {
                        return Arc::clone(font);
                    }
                }
                match weight {
                    FontWeight::Bold => Arc::clone(&self.sans_bold),
                    FontWeight::Regular => Arc::clone(&self.sans_regular),
                }
            }
        }
    }

    /// Selects a font face that contains a glyph for `ch`, falling back to platform Indic/CJK/symbol fonts.
    pub fn select_font_for_char(
        &self,
        family: FontFamily,
        weight: FontWeight,
        ch: char,
    ) -> Arc<Font> {
        let primary = self.select_font(family, weight);
        if ch.is_ascii_whitespace() || primary.lookup_glyph_index(ch) != 0 {
            return primary;
        }

        if let FontFamily::Custom(id) = family {
            if let Ok(web_fonts) = self.web_fonts.read()
                && let Some(entry) = web_fonts.get(&id)
            {
                let faces = match weight {
                    FontWeight::Bold => entry.bold.iter().chain(entry.regular.iter()),
                    FontWeight::Regular => entry.regular.iter().chain(entry.bold.iter()),
                };
                for face in faces {
                    if face.lookup_glyph_index(ch) != 0 {
                        return Arc::clone(face);
                    }
                }
            }
        }

        if let Ok(missing) = self.missing_glyphs.read()
            && missing.contains(&ch)
        {
            return primary;
        }

        if let Ok(loaded) = self.fallback_fonts.read() {
            for fb in loaded.iter() {
                if fb.lookup_glyph_index(ch) != 0 {
                    return Arc::clone(fb);
                }
            }
        }

        let script_candidates = script_candidates_for_char(ch);
        if let Ok(mut pending) = self.unloaded_fallbacks.write() {
            let mut found_font = None;

            if !script_candidates.is_empty() {
                for &target_name in script_candidates {
                    if let Some(pos) = pending
                        .iter()
                        .position(|&name| name.eq_ignore_ascii_case(target_name))
                    {
                        let name = pending.remove(pos);
                        if let Some(font) = load_system_font(name) {
                            let font = Arc::new(font);
                            if let Ok(mut loaded) = self.fallback_fonts.write() {
                                loaded.push(Arc::clone(&font));
                            }
                            if font.lookup_glyph_index(ch) != 0 {
                                found_font = Some(font);
                                break;
                            }
                        }
                    }
                }
            }

            if found_font.is_none() {
                let mut remaining = Vec::with_capacity(pending.len());
                for name in pending.drain(..) {
                    if found_font.is_none() {
                        if let Some(font) = load_system_font(name) {
                            let font = Arc::new(font);
                            if let Ok(mut loaded) = self.fallback_fonts.write() {
                                loaded.push(Arc::clone(&font));
                            }
                            if font.lookup_glyph_index(ch) != 0 {
                                found_font = Some(font);
                                continue;
                            }
                        }
                    } else {
                        remaining.push(name);
                    }
                }
                *pending = remaining;
            }

            if let Some(fb) = found_font {
                return fb;
            }
        }

        if let Ok(mut missing) = self.missing_glyphs.write() {
            missing.insert(ch);
        }

        primary
    }

    /// Returns the font's (ascent, descent, line_gap) in pixels at the specified font size.
    pub fn font_metrics(
        &self,
        family: FontFamily,
        weight: FontWeight,
        font_size: f32,
    ) -> (f32, f32, f32) {
        let font = self.select_font(family, weight);
        let px_size = font_size.max(1.0);
        if let Some(lm) = font.horizontal_line_metrics(px_size) {
            (lm.ascent, lm.descent.abs(), lm.line_gap)
        } else {
            (px_size * 0.8, px_size * 0.2, 0.0)
        }
    }

    /// Registers a downloadable web font (WOFF2, WOFF1, TrueType, or OpenType) into the font manager.
    pub fn register_web_font(
        &self,
        family_name: &str,
        weight: FontWeight,
        font_data: &[u8],
    ) -> Result<u32, String> {
        let raw_bytes: Cow<[u8]> = if font_data.starts_with(b"data:") {
            if let Ok(data_str) = std::str::from_utf8(font_data) {
                if let Some(comma_idx) = data_str.find(',') {
                    let metadata = &data_str[..comma_idx];
                    let payload = &data_str[comma_idx + 1..];
                    if metadata.contains(";base64") {
                        if let Some(dec) = crate::image_decode::base64_decode(payload) {
                            Cow::Owned(dec)
                        } else {
                            return Err("Invalid base64 encoding in font data URI".to_string());
                        }
                    } else {
                        Cow::Borrowed(payload.as_bytes())
                    }
                } else {
                    return Err("Invalid font data URI: missing comma".to_string());
                }
            } else {
                return Err("Invalid UTF-8 in font data URI".to_string());
            }
        } else {
            Cow::Borrowed(font_data)
        };

        let key = family_name
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_ascii_lowercase();

        let mut family_map = self.family_to_id.write().map_err(|e| e.to_string())?;
        let mut web_fonts = self.web_fonts.write().map_err(|e| e.to_string())?;

        if let Some(&existing_id) = family_map.get(&key) {
            if let Some(entry) = web_fonts.get(&existing_id) {
                let already_has = match weight {
                    FontWeight::Bold => !entry.bold.is_empty(),
                    FontWeight::Regular => !entry.regular.is_empty(),
                };
                if already_has {
                    return Ok(existing_id);
                }
            }
        }

        let font = Arc::new(decode_font_bytes(&raw_bytes)?);

        let id = if let Some(&existing_id) = family_map.get(&key) {
            if let Some(entry) = web_fonts.get_mut(&existing_id) {
                entry.last_accessed.store(
                    FONT_ACCESS_TICK.fetch_add(1, Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                match weight {
                    FontWeight::Bold => entry.bold.push(Arc::clone(&font)),
                    FontWeight::Regular => entry.regular.push(Arc::clone(&font)),
                }
            }
            existing_id
        } else {
            if web_fonts.len() >= MAX_WEB_FONT_FAMILIES {
                let oldest_id = web_fonts
                    .iter()
                    .filter(|(_, entry)| !entry.is_system)
                    .min_by_key(|(_, entry)| entry.last_accessed.load(Ordering::Relaxed))
                    .map(|(id, _)| *id);
                if let Some(old_id) = oldest_id {
                    if let Some(removed) = web_fonts.remove(&old_id) {
                        let key = removed
                            .family_name
                            .trim()
                            .trim_matches('\'')
                            .trim_matches('"')
                            .to_ascii_lowercase();
                        family_map.remove(&key);
                        drop(removed);
                    }
                }
            }

            let new_id = self.next_font_id.fetch_add(1, Ordering::Relaxed);
            let mut entry = WebFontEntry {
                family_name: family_name.to_string(),
                regular: Vec::new(),
                bold: Vec::new(),
                is_system: false,
                last_accessed: std::sync::atomic::AtomicU64::new(
                    FONT_ACCESS_TICK.fetch_add(1, Ordering::Relaxed),
                ),
            };
            match weight {
                FontWeight::Bold => entry.bold.push(Arc::clone(&font)),
                FontWeight::Regular => entry.regular.push(Arc::clone(&font)),
            }
            family_map.insert(key, new_id);
            web_fonts.insert(new_id, entry);
            new_id
        };

        bump_web_font_epoch();
        Ok(id)
    }

    /// Checks if a custom web font family has been registered.
    pub fn has_web_font(&self, family_name: &str) -> bool {
        let key = family_name
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_ascii_lowercase();
        self.family_to_id
            .read()
            .map(|m| m.contains_key(&key))
            .unwrap_or(false)
    }

    /// Checks if a specific weight for a custom web font family has been registered.
    pub fn has_web_font_weight(&self, family_name: &str, weight: FontWeight) -> bool {
        let key = family_name
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_ascii_lowercase();
        if let Ok(family_map) = self.family_to_id.read()
            && let Some(&id) = family_map.get(&key)
            && let Ok(web_fonts) = self.web_fonts.read()
            && let Some(entry) = web_fonts.get(&id)
        {
            return match weight {
                FontWeight::Bold => !entry.bold.is_empty(),
                FontWeight::Regular => !entry.regular.is_empty(),
            };
        }
        false
    }

    /// Returns human-readable description of a font family including loaded weights.
    pub fn debug_family_name(&self, family: FontFamily) -> String {
        match family {
            FontFamily::SansSerif => "SansSerif".to_string(),
            FontFamily::Serif => "Serif".to_string(),
            FontFamily::Monospace => "Monospace".to_string(),
            FontFamily::Custom(id) => {
                if let Ok(web_fonts) = self.web_fonts.read()
                    && let Some(entry) = web_fonts.get(&id)
                {
                    return format!(
                        "{}(has_reg={}, has_bold={})",
                        entry.family_name,
                        !entry.regular.is_empty(),
                        !entry.bold.is_empty()
                    );
                }
                format!("Custom({})", id)
            }
        }
    }

    /// Maps a CSS `font-family` string to a `FontFamily` enum via the global FontManager.
    pub fn resolve_family(family_str: &str) -> FontFamily {
        font_manager().resolve_family_for(family_str)
    }

    /// Maps a CSS `font-family` string to a `FontFamily` enum, prioritizing registered web fonts
    /// and named platform system fonts (`Verdana`, `Segoe UI`, `Arial`, etc.).
    pub fn resolve_family_for(&self, family_str: &str) -> FontFamily {
        for item in family_str.split(',') {
            let mut clean = item
                .trim()
                .trim_matches('\'')
                .trim_matches('"')
                .to_ascii_lowercase();
            if clean.is_empty() {
                continue;
            }

            if matches!(
                clean.as_str(),
                "system-ui" | "-apple-system" | "blinkmacsystemfont"
            ) {
                clean = "segoe ui".to_string();
            }

            if let Ok(family_map) = self.family_to_id.read()
                && let Some(&id) = family_map.get(&clean)
            {
                return FontFamily::Custom(id);
            }

            if clean == "sans-serif" {
                return FontFamily::SansSerif;
            } else if clean == "serif" {
                return FontFamily::Serif;
            } else if clean == "monospace" {
                return FontFamily::Monospace;
            } else if clean == "cursive" || clean == "fantasy" {
                return FontFamily::SansSerif;
            }

            let system_files = match clean.as_str() {
                "verdana" => Some(("verdana.ttf", "verdanab.ttf")),
                "segoe ui" | "segoe" => Some(("segoeui.ttf", "segoeuib.ttf")),
                "arial" | "helvetica" => Some(("arial.ttf", "arialbd.ttf")),
                "times new roman" | "times" => Some(("times.ttf", "timesbd.ttf")),
                "consolas" => Some(("consola.ttf", "consolab.ttf")),
                "georgia" => Some(("georgia.ttf", "georgiab.ttf")),
                "tahoma" => Some(("tahoma.ttf", "tahomabd.ttf")),
                "trebuchet ms" | "trebuchet" => Some(("trebuc.ttf", "trebucbd.ttf")),
                "calibri" => Some(("calibri.ttf", "calibrib.ttf")),
                "courier new" | "courier" => Some(("cour.ttf", "courbd.ttf")),
                _ => None,
            };

            if let Some((reg_file, bold_file)) = system_files {
                if clean == "arial" || clean == "helvetica" {
                    return FontFamily::SansSerif;
                }
                if clean == "times new roman" || clean == "times" {
                    return FontFamily::Serif;
                }
                if clean == "courier new" || clean == "courier" {
                    return FontFamily::Monospace;
                }

                let mut family_map = match self.family_to_id.write() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if let Some(&id) = family_map.get(&clean) {
                    return FontFamily::Custom(id);
                }

                if let Some(reg_font) = load_system_font(reg_file) {
                    let reg_font = Arc::new(reg_font);
                    let bold_font: Option<Arc<Font>> = load_system_font(bold_file).map(Arc::new);

                    let new_id = self.next_font_id.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut web_fonts) = self.web_fonts.write() {
                        family_map.insert(clean.clone(), new_id);
                        if clean == "segoe ui" {
                            family_map.insert("segoe".to_string(), new_id);
                            family_map.insert("system-ui".to_string(), new_id);
                            family_map.insert("-apple-system".to_string(), new_id);
                            family_map.insert("blinkmacsystemfont".to_string(), new_id);
                        }
                        let bold_vec = if let Some(b) = bold_font {
                            vec![b]
                        } else {
                            Vec::new()
                        };
                        web_fonts.insert(
                            new_id,
                            WebFontEntry {
                                family_name: clean,
                                regular: vec![Arc::clone(&reg_font)],
                                bold: bold_vec,
                                is_system: true,
                                last_accessed: std::sync::atomic::AtomicU64::new(
                                    FONT_ACCESS_TICK.fetch_add(1, Ordering::Relaxed),
                                ),
                            },
                        );
                        return FontFamily::Custom(new_id);
                    }
                }
            }
        }

        let lower = family_str.to_lowercase();
        if lower.contains("mono") || lower.contains("courier") || lower.contains("consolas") {
            FontFamily::Monospace
        } else if (lower.contains("serif") && !lower.contains("sans"))
            || lower.contains("times")
            || lower.contains("georgia")
            || lower.contains("palatino")
            || lower.contains("cambria")
        {
            FontFamily::Serif
        } else {
            FontFamily::SansSerif
        }
    }

    /// Prunes custom web fonts loaded across navigations, reclaiming heap allocations.
    pub fn prune_web_fonts(&self) {
        if let Ok(mut family_map) = self.family_to_id.write()
            && let Ok(mut web_fonts) = self.web_fonts.write()
        {
            let to_remove: Vec<(u32, String)> = web_fonts
                .iter()
                .filter(|(_, entry)| !entry.is_system)
                .map(|(id, entry)| (*id, entry.family_name.clone()))
                .collect();

            for (id, family_name) in to_remove {
                let key = family_name
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_ascii_lowercase();
                family_map.remove(&key);
                if let Some(removed) = web_fonts.remove(&id) {
                    drop(removed);
                }
            }
            bump_web_font_epoch();
        }
    }

    /// Returns the current generation counter of loaded web fonts.
    pub fn web_font_epoch(&self) -> u64 {
        web_font_epoch()
    }

    /// Measures the total width and height of a text string at the given font size.
    pub fn measure_text(
        &self,
        text: &str,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
    ) -> (f32, f32) {
        self.measure_text_with_spacing(text, font_size, weight, family, 0.0)
    }

    /// Measures the total width and height of a text string at the given font size,
    /// adding `letter_spacing` after each character.
    pub fn measure_text_with_spacing(
        &self,
        text: &str,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        letter_spacing: f32,
    ) -> (f32, f32) {
        let primary_font = self.select_font(family, weight);
        let reg_font = self.select_font(family, FontWeight::Regular);
        let is_synth_bold = weight == FontWeight::Bold;
        let px_size = font_size.max(1.0);

        let shaped_text;
        let text_to_measure = if text.chars().any(super::shaping::is_complex_char) {
            shaped_text = super::shaping::shape_complex_script(text);
            &shaped_text
        } else {
            text
        };

        let mut total_width = 0.0f32;
        for ch in text_to_measure.chars() {
            if ch == '\n' {
                continue;
            }
            if super::emoji::is_zero_width_or_format(ch) {
                continue;
            }
            if is_emoji_char(ch) {
                total_width += px_size + letter_spacing;
                continue;
            }
            let font = self.select_font_for_char(family, weight, ch);
            let metrics = font.metrics(ch, px_size);
            let synth_extra = if is_synth_bold
                && std::ptr::eq(font.as_ref(), reg_font.as_ref())
                && !ch.is_whitespace()
            {
                if px_size >= 48.0 {
                    3.0
                } else if px_size >= 28.0 {
                    2.0
                } else {
                    1.0
                }
            } else {
                0.0
            };
            total_width += metrics.advance_width + synth_extra + letter_spacing;
        }

        let line_metrics = primary_font.horizontal_line_metrics(px_size);
        let height = line_metrics
            .map(|lm| lm.new_line_size)
            .unwrap_or(px_size * 1.2);

        (total_width, height)
    }

    /// Measures text using default SansSerif family (for UI elements).
    pub fn measure_text_sans(&self, text: &str, font_size: f32, weight: FontWeight) -> (f32, f32) {
        self.measure_text(text, font_size, weight, FontFamily::SansSerif)
    }

    /// Measures the advance width of a single character.
    pub fn measure_char_width(
        &self,
        ch: char,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
    ) -> f32 {
        if super::emoji::is_zero_width_or_format(ch) {
            return 0.0;
        }
        if is_emoji_char(ch) {
            return font_size.max(1.0);
        }
        let font = self.select_font_for_char(family, weight, ch);
        let metrics = font.metrics(ch, font_size.max(1.0));
        let synth_extra = if weight == FontWeight::Bold
            && std::ptr::eq(
                font.as_ref(),
                self.select_font(family, FontWeight::Regular).as_ref(),
            )
            && !ch.is_whitespace()
        {
            if font_size >= 28.0 { 2.0 } else { 1.0 }
        } else {
            0.0
        };
        metrics.advance_width + synth_extra
    }
}

// SAFETY: FontManager contains thread-safe types (`Arc<Font>`, `RwLock<...>`, `AtomicU32`).
// `fontdue::Font` is immutable and safe to share across threads, and we never mutate fonts after construction.
unsafe impl Send for FontManager {}
// SAFETY: All internal mutable state is synchronized via `RwLock` or `AtomicU32`.
unsafe impl Sync for FontManager {}
