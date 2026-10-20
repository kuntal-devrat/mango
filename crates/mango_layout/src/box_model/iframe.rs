//! Security and capability restrictions applied to an `<iframe>` nested browsing context.
//!
//! Implements standard WHATWG HTML5 sandboxing flags:
//! - `allow-scripts`
//! - `allow-same-origin`
//! - `allow-forms`
//! - `allow-top-navigation`
//! - `allow-top-navigation-by-user-activation`
//! - `allow-popups`
//! - `allow-popups-to-escape-sandbox`
//! - `allow-modals`
//! - `allow-downloads`
//! - `allow-pointer-lock`
//! - `allow-orientation-lock`
//! - `allow-presentation`

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IFrameSandbox {
    pub is_sandboxed: bool,
    pub allow_scripts: bool,
    pub allow_same_origin: bool,
    pub allow_forms: bool,
    pub allow_top_navigation: bool,
    pub allow_top_navigation_by_user_activation: bool,
    pub allow_popups: bool,
    pub allow_popups_to_escape_sandbox: bool,
    pub allow_modals: bool,
    pub allow_downloads: bool,
    pub allow_pointer_lock: bool,
    pub allow_orientation_lock: bool,
    pub allow_presentation: bool,
}

impl Default for IFrameSandbox {
    fn default() -> Self {
        Self {
            is_sandboxed: false,
            allow_scripts: true,
            allow_same_origin: true,
            allow_forms: true,
            allow_top_navigation: true,
            allow_top_navigation_by_user_activation: true,
            allow_popups: true,
            allow_popups_to_escape_sandbox: true,
            allow_modals: true,
            allow_downloads: true,
            allow_pointer_lock: true,
            allow_orientation_lock: true,
            allow_presentation: true,
        }
    }
}

impl IFrameSandbox {
    /// Creates a fully restricted sandbox (all permissions disabled).
    pub fn restricted() -> Self {
        Self {
            is_sandboxed: true,
            allow_scripts: false,
            allow_same_origin: false,
            allow_forms: false,
            allow_top_navigation: false,
            allow_top_navigation_by_user_activation: false,
            allow_popups: false,
            allow_popups_to_escape_sandbox: false,
            allow_modals: false,
            allow_downloads: false,
            allow_pointer_lock: false,
            allow_orientation_lock: false,
            allow_presentation: false,
        }
    }

    /// Parses the `sandbox` attribute value into an [`IFrameSandbox`].
    pub fn from_attribute(attr: Option<&str>) -> Self {
        let Some(val) = attr else {
            return Self::default();
        };

        let mut sandbox = Self::restricted();
        let tokens: Vec<&str> = val.split_ascii_whitespace().collect();

        for token in tokens {
            match token.to_ascii_lowercase().as_str() {
                "allow-scripts" => sandbox.allow_scripts = true,
                "allow-same-origin" => sandbox.allow_same_origin = true,
                "allow-forms" => sandbox.allow_forms = true,
                "allow-top-navigation" => sandbox.allow_top_navigation = true,
                "allow-top-navigation-by-user-activation" => {
                    sandbox.allow_top_navigation_by_user_activation = true;
                }
                "allow-popups" => sandbox.allow_popups = true,
                "allow-popups-to-escape-sandbox" => {
                    sandbox.allow_popups_to_escape_sandbox = true;
                }
                "allow-modals" => sandbox.allow_modals = true,
                "allow-downloads" => sandbox.allow_downloads = true,
                "allow-pointer-lock" => sandbox.allow_pointer_lock = true,
                "allow-orientation-lock" => sandbox.allow_orientation_lock = true,
                "allow-presentation" => sandbox.allow_presentation = true,
                _ => {}
            }
        }

        sandbox
    }

    /// Serializes the sandbox back to an attribute string (canonical order).
    pub fn to_attribute_value(&self) -> String {
        self.to_attribute()
    }

    /// Serializes the sandbox back to an attribute string (canonical order).
    pub fn to_attribute(&self) -> String {
        if !self.is_sandboxed {
            return String::new();
        }
        let mut tokens = Vec::new();
        if self.allow_scripts {
            tokens.push("allow-scripts");
        }
        if self.allow_same_origin {
            tokens.push("allow-same-origin");
        }
        if self.allow_forms {
            tokens.push("allow-forms");
        }
        if self.allow_top_navigation {
            tokens.push("allow-top-navigation");
        }
        if self.allow_top_navigation_by_user_activation {
            tokens.push("allow-top-navigation-by-user-activation");
        }
        if self.allow_popups {
            tokens.push("allow-popups");
        }
        if self.allow_popups_to_escape_sandbox {
            tokens.push("allow-popups-to-escape-sandbox");
        }
        if self.allow_modals {
            tokens.push("allow-modals");
        }
        if self.allow_downloads {
            tokens.push("allow-downloads");
        }
        if self.allow_pointer_lock {
            tokens.push("allow-pointer-lock");
        }
        if self.allow_orientation_lock {
            tokens.push("allow-orientation-lock");
        }
        if self.allow_presentation {
            tokens.push("allow-presentation");
        }
        tokens.join(" ")
    }
}
