use mango_html::dom::ElementData;

/// Accessible states and properties mapped from both ARIA attributes and native
/// HTML attributes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11yState {
    pub disabled: bool,
    pub hidden: bool,
    pub expanded: Option<bool>,
    pub checked: Option<CheckedState>,
    pub pressed: Option<PressedState>,
    pub selected: Option<bool>,
    pub readonly: bool,
    pub required: bool,
    pub focusable: bool,
    pub invalid: Option<String>,
    pub busy: bool,
    pub modal: bool,
    pub has_popup: Option<String>,
    pub autocomplete: Option<String>,
    pub current: Option<String>,
    pub orientation: Option<String>,
    pub multiselectable: bool,
    pub multiline: bool,

    // Value properties
    pub value: Option<String>,
    pub value_text: Option<String>,
    pub value_min: Option<f32>,
    pub value_max: Option<f32>,
    pub value_now: Option<f32>,

    // Live region properties
    pub live: Option<String>,
    pub atomic: Option<bool>,
    pub relevant: Option<String>,

    // Relationship properties (stored as raw ID strings)
    pub controls: Vec<String>,
    pub owns: Vec<String>,
    pub flowto: Vec<String>,
    pub active_descendant: Option<String>,
    pub error_message: Option<String>,

    // Set/position properties
    pub pos_in_set: Option<u32>,
    pub set_size: Option<u32>,
    pub level: Option<u32>,

    // Table properties
    pub col_count: Option<u32>,
    pub col_index: Option<u32>,
    pub col_span: Option<u32>,
    pub row_count: Option<u32>,
    pub row_index: Option<u32>,
    pub row_span: Option<u32>,

    // Custom role description
    pub role_description: Option<String>,
    pub key_shortcuts: Option<String>,
}

/// Tri-state checked (for checkboxes).
#[derive(Debug, Clone, PartialEq)]
pub enum CheckedState {
    True,
    False,
    Mixed,
}

/// Tri-state pressed (for toggle buttons).
#[derive(Debug, Clone, PartialEq)]
pub enum PressedState {
    True,
    False,
    Mixed,
}

/// Resolves accessible state and properties from attributes.
pub fn resolve_state(elem: &ElementData, tag: &str) -> A11yState {
    let mut state = A11yState::default();

    // Disabled
    state.disabled = elem.get_attribute("disabled").is_some()
        || elem.get_attribute("aria-disabled") == Some("true");

    // Read-only
    state.readonly = elem.get_attribute("readonly").is_some()
        || elem.get_attribute("aria-readonly") == Some("true");

    // Required
    state.required = elem.get_attribute("required").is_some()
        || elem.get_attribute("aria-required") == Some("true");

    // Busy
    state.busy = elem.get_attribute("aria-busy") == Some("true");

    // Modal
    state.modal = elem.get_attribute("aria-modal") == Some("true");

    // Multiselectable
    state.multiselectable = elem.get_attribute("aria-multiselectable") == Some("true")
        || (tag == "select" && elem.get_attribute("multiple").is_some());

    // Multiline
    state.multiline = elem.get_attribute("aria-multiline") == Some("true") || tag == "textarea";

    // Expanded (also handles <details open>)
    if let Some(v) = elem.get_attribute("aria-expanded") {
        state.expanded = Some(v == "true");
    } else if tag == "details" {
        state.expanded = Some(elem.get_attribute("open").is_some());
    }

    // Checked (tri-state: true/false/mixed)
    if let Some(v) = elem.get_attribute("aria-checked") {
        state.checked = Some(match v {
            "true" => CheckedState::True,
            "mixed" => CheckedState::Mixed,
            _ => CheckedState::False,
        });
    } else if elem.get_attribute("checked").is_some() {
        state.checked = Some(CheckedState::True);
    }

    // Pressed (tri-state for toggle buttons)
    if let Some(v) = elem.get_attribute("aria-pressed") {
        state.pressed = Some(match v {
            "true" => PressedState::True,
            "mixed" => PressedState::Mixed,
            _ => PressedState::False,
        });
    }

    // Selected
    if let Some(v) = elem.get_attribute("aria-selected") {
        state.selected = Some(v == "true");
    } else if tag == "option" && elem.get_attribute("selected").is_some() {
        state.selected = Some(true);
    }

    // Invalid
    if let Some(v) = elem.get_attribute("aria-invalid")
        && v != "false"
    {
        state.invalid = Some(v.to_string());
    }

    // Has popup
    if let Some(v) = elem.get_attribute("aria-haspopup")
        && v != "false"
    {
        state.has_popup = Some(v.to_string());
    }

    // Autocomplete
    if let Some(v) = elem
        .get_attribute("aria-autocomplete")
        .or_else(|| elem.get_attribute("autocomplete"))
    {
        state.autocomplete = Some(v.to_string());
    }

    // Current
    if let Some(v) = elem.get_attribute("aria-current")
        && v != "false"
    {
        state.current = Some(v.to_string());
    }

    // Orientation
    state.orientation = elem
        .get_attribute("aria-orientation")
        .map(|s| s.to_string());

    // Value (native + ARIA)
    if let Some(v) = elem.get_attribute("value") {
        state.value = Some(v.to_string());
    }
    state.value_text = elem.get_attribute("aria-valuetext").map(|s| s.to_string());
    state.value_min = elem
        .get_attribute("aria-valuemin")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("min").and_then(|v| v.parse().ok()));
    state.value_max = elem
        .get_attribute("aria-valuemax")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("max").and_then(|v| v.parse().ok()));
    state.value_now = elem
        .get_attribute("aria-valuenow")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("value").and_then(|v| v.parse().ok()));

    // Live region properties
    state.live = elem.get_attribute("aria-live").map(|s| s.to_string());
    if let Some(v) = elem.get_attribute("aria-atomic") {
        state.atomic = Some(v == "true");
    }
    state.relevant = elem.get_attribute("aria-relevant").map(|s| s.to_string());

    // Relationship IDs
    state.controls = split_ids(elem.get_attribute("aria-controls"));
    state.owns = split_ids(elem.get_attribute("aria-owns"));
    state.flowto = split_ids(elem.get_attribute("aria-flowto"));
    state.active_descendant = elem
        .get_attribute("aria-activedescendant")
        .map(|s| s.to_string());
    state.error_message = elem
        .get_attribute("aria-errormessage")
        .map(|s| s.to_string());

    // Set/position
    state.pos_in_set = elem
        .get_attribute("aria-posinset")
        .and_then(|v| v.parse().ok());
    state.set_size = elem
        .get_attribute("aria-setsize")
        .and_then(|v| v.parse().ok());
    state.level = elem
        .get_attribute("aria-level")
        .and_then(|v| v.parse().ok());

    // Table properties
    state.col_count = elem
        .get_attribute("aria-colcount")
        .and_then(|v| v.parse().ok());
    state.col_index = elem
        .get_attribute("aria-colindex")
        .and_then(|v| v.parse().ok());
    state.col_span = elem
        .get_attribute("aria-colspan")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("colspan").and_then(|v| v.parse().ok()));
    state.row_count = elem
        .get_attribute("aria-rowcount")
        .and_then(|v| v.parse().ok());
    state.row_index = elem
        .get_attribute("aria-rowindex")
        .and_then(|v| v.parse().ok());
    state.row_span = elem
        .get_attribute("aria-rowspan")
        .and_then(|v| v.parse().ok())
        .or_else(|| elem.get_attribute("rowspan").and_then(|v| v.parse().ok()));

    // Role description & key shortcuts
    state.role_description = elem
        .get_attribute("aria-roledescription")
        .map(|s| s.to_string());
    state.key_shortcuts = elem
        .get_attribute("aria-keyshortcuts")
        .map(|s| s.to_string());

    // Focusable heuristic
    state.focusable = is_natively_focusable(tag, elem) || elem.get_attribute("tabindex").is_some();

    state
}

/// Determines if an element is natively focusable (without tabindex).
pub fn is_natively_focusable(tag: &str, elem: &ElementData) -> bool {
    match tag {
        "a" | "area" => elem.get_attribute("href").is_some(),
        "button" | "textarea" | "select" | "details" | "summary" => true,
        "input" => {
            let t = elem.get_attribute("type").unwrap_or("text");
            !t.eq_ignore_ascii_case("hidden")
        }
        _ => false,
    }
}

/// Splits a space-separated ID reference list into a Vec of Strings.
pub fn split_ids(attr: Option<&str>) -> Vec<String> {
    attr.map(|s| s.split_whitespace().map(|id| id.to_string()).collect())
        .unwrap_or_default()
}
