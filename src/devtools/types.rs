use mango_html::dom::NodeId;

/// Active panel tab inside DevTools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsTab {
    Elements,
    Console,
    Network,
    Sources,
    Computed,
    Performance,
    Debugger,
}

/// Log severity in the DevTools console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
}

/// A captured console message.
#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub level: ConsoleLevel,
    pub message: String,
    pub time_str: String,
}

/// A recorded network request.
#[derive(Debug, Clone)]
pub struct NetworkEntry {
    pub url: String,
    pub method: String,
    pub status: u16,
    pub mime_type: String,
    pub size_bytes: usize,
}

/// Performance mark or measure trace entry.
#[derive(Debug, Clone)]
pub struct PerformanceMetric {
    pub name: String,
    pub start_time_ms: f64,
    pub duration_ms: f64,
    pub category: String,
}

/// A debugger breakpoint.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: u32,
    pub script_url: String,
    pub line_number: u32,
    pub enabled: bool,
}

/// JavaScript execution call stack frame.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: String,
    pub script_url: String,
    pub line_number: u32,
    pub column_number: u32,
}

/// Built-in Developer Tools state.
#[derive(Debug, Clone)]
pub struct DevTools {
    pub is_open: bool,
    pub active_tab: DevToolsTab,
    pub height: f32,
    pub selected_node_id: Option<NodeId>,
    pub hovered_node_id: Option<NodeId>,
    pub inspect_mode: bool,
    pub console_entries: Vec<ConsoleEntry>,
    pub console_scroll: usize,
    pub network_entries: Vec<NetworkEntry>,
    pub performance_metrics: Vec<PerformanceMetric>,
    pub breakpoints: Vec<Breakpoint>,
    pub call_stack: Vec<StackFrame>,
    pub is_paused: bool,
}

impl Default for DevTools {
    fn default() -> Self {
        Self::new()
    }
}

impl DevTools {
    /// Creates a new DevTools instance (hidden by default).
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: DevToolsTab::Elements,
            height: 260.0,
            selected_node_id: None,
            hovered_node_id: None,
            inspect_mode: false,
            console_entries: Vec::new(),
            console_scroll: 0,
            network_entries: Vec::new(),
            performance_metrics: Vec::new(),
            breakpoints: Vec::new(),
            call_stack: Vec::new(),
            is_paused: false,
        }
    }

    /// Toggles the visibility of DevTools.
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Opens DevTools to a specific tab.
    pub fn open_tab(&mut self, tab: DevToolsTab) {
        self.is_open = true;
        self.active_tab = tab;
    }

    /// Appends a message to the console panel.
    pub fn log(&mut self, level: ConsoleLevel, msg: impl Into<String>) {
        if self.console_entries.len() >= 500 {
            self.console_entries.remove(0);
        }
        self.console_entries.push(ConsoleEntry {
            level,
            message: msg.into(),
            time_str: "now".to_string(),
        });
        self.console_scroll = 0;
    }

    pub fn clear_console(&mut self) {
        self.console_entries.clear();
        self.console_scroll = 0;
    }

    pub fn scroll_console(&mut self, delta: i32) {
        let max = self.console_entries.len().saturating_sub(1);
        self.console_scroll = (self.console_scroll as i32 + delta).clamp(0, max as i32) as usize;
    }

    /// Records an HTTP network transaction.
    pub fn record_network(
        &mut self,
        url: impl Into<String>,
        method: impl Into<String>,
        status: u16,
        mime_type: impl Into<String>,
        size_bytes: usize,
    ) {
        if self.network_entries.len() >= 200 {
            self.network_entries.remove(0);
        }
        self.network_entries.push(NetworkEntry {
            url: url.into(),
            method: method.into(),
            status,
            mime_type: mime_type.into(),
            size_bytes,
        });
    }

    /// Selects a DOM node for style and element inspection.
    pub fn select_node(&mut self, node_id: Option<NodeId>) {
        self.selected_node_id = node_id;
    }

    /// Records a performance mark or measurement.
    pub fn record_metric(
        &mut self,
        name: impl Into<String>,
        start_time_ms: f64,
        duration_ms: f64,
        category: impl Into<String>,
    ) {
        self.performance_metrics.push(PerformanceMetric {
            name: name.into(),
            start_time_ms,
            duration_ms,
            category: category.into(),
        });
    }

    /// Adds a breakpoint for debugging.
    pub fn add_breakpoint(&mut self, script_url: impl Into<String>, line_number: u32) -> u32 {
        let id = self.breakpoints.len() as u32 + 1;
        self.breakpoints.push(Breakpoint {
            id,
            script_url: script_url.into(),
            line_number,
            enabled: true,
        });
        id
    }

    /// Updates the JavaScript debugger call stack.
    pub fn set_call_stack(&mut self, stack: Vec<StackFrame>) {
        self.call_stack = stack;
    }
}
