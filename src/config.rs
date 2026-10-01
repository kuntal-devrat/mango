//! User configuration.
//!
//! Loads configuration settings for Mango browser.
//! Default fallback is provided when config file is absent.

/// Browser configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// The home page URL.
    pub homepage: String,
    /// Default font size in pixels.
    pub default_font_size: f32,
    /// Search engine query URL template (with `%s` placeholder).
    pub search_url: String,
    /// Default window width in pixels.
    pub window_width: u32,
    /// Default window height in pixels.
    pub window_height: u32,
}

impl Config {
    pub fn new() -> Self {
        Self::load()
    }

    /// Loads configuration from `mango.toml` in the current working directory,
    /// falling back to defaults if not found or on parse failure.
    pub fn load() -> Self {
        if let Ok(content) = std::fs::read_to_string("mango.toml") {
            Self::parse_toml(&content)
        } else {
            Self::default()
        }
    }

    /// Loads configuration from an explicit file path.
    pub fn load_from_path(path: &std::path::Path) -> Self {
        if let Ok(content) = std::fs::read_to_string(path) {
            Self::parse_toml(&content)
        } else {
            Self::default()
        }
    }

    /// Formats a search query URL using the configured search URL template.
    pub fn format_search_url(&self, query: &str) -> String {
        let encoded: String = query
            .bytes()
            .map(|b| match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                b' ' => "+".to_string(),
                _ => format!("%{:02X}", b),
            })
            .collect();
        if self.search_url.contains("%s") {
            self.search_url.replace("%s", &encoded)
        } else {
            format!("{}{}", self.search_url, encoded)
        }
    }

    fn parse_toml(content: &str) -> Self {
        let mut cfg = Self::default();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                match key {
                    "homepage" => cfg.homepage = val.to_string(),
                    "search_url" => cfg.search_url = val.to_string(),
                    "default_font_size" => {
                        if let Ok(sz) = val.parse::<f32>() {
                            cfg.default_font_size = sz;
                        }
                    }
                    "window_width" => {
                        if let Ok(w) = val.parse::<u32>() {
                            cfg.window_width = w;
                        }
                    }
                    "window_height" => {
                        if let Ok(h) = val.parse::<u32>() {
                            cfg.window_height = h;
                        }
                    }
                    _ => {}
                }
            }
        }
        cfg
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            homepage: "about:welcome".to_string(),
            default_font_size: 16.0,
            search_url: "https://html.duckduckgo.com/html/?q=%s".to_string(),
            window_width: 1280,
            window_height: 900,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_and_parsing() {
        let toml = r#"
            # Mango Browser Configuration
            homepage = "https://example.com"
            search_url = "https://search.brave.com/search?q=%s"
            default_font_size = 18.0
            window_width = 1440
            window_height = 960
        "#;
        let cfg = Config::parse_toml(toml);
        assert_eq!(cfg.homepage, "https://example.com");
        assert_eq!(cfg.search_url, "https://search.brave.com/search?q=%s");
        assert_eq!(cfg.default_font_size, 18.0);
        assert_eq!(cfg.window_width, 1440);
        assert_eq!(cfg.window_height, 960);

        let search_req = cfg.format_search_url("hello world");
        assert_eq!(search_req, "https://search.brave.com/search?q=hello+world");
    }
}
