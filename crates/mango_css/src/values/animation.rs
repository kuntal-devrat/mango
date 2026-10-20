/// CSS `<step-position>` values for `steps()` timing functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepPosition {
    #[default]
    JumpEnd,
    JumpStart,
    JumpNone,
    JumpBoth,
}

/// CSS `<easing-function>`: how an animation/transition progresses over time.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TimingFunction {
    Linear,
    #[default]
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// `cubic-bezier(x1, y1, x2, y2)`
    CubicBezier(f32, f32, f32, f32),
    /// `steps(n, position)`
    Steps(u32, StepPosition),
}

impl TimingFunction {
    /// Maps an `input` progress in `[0, 1]` to an eased output progress in `[0, 1]`.
    pub fn sample(&self, input: f32) -> f32 {
        let t = input.clamp(0.0, 1.0);
        match self {
            TimingFunction::Linear => t,
            TimingFunction::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            TimingFunction::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
            TimingFunction::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
            TimingFunction::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
            TimingFunction::CubicBezier(x1, y1, x2, y2) => cubic_bezier(*x1, *y1, *x2, *y2, t),
            TimingFunction::Steps(n, pos) => {
                let steps = (*n).max(1) as f32;
                match pos {
                    StepPosition::JumpStart => ((t * steps).ceil() / steps).clamp(0.0, 1.0),
                    StepPosition::JumpEnd => ((t * steps).floor() / steps).clamp(0.0, 1.0),
                    StepPosition::JumpNone => {
                        if t <= 0.0 || t >= 1.0 {
                            t
                        } else {
                            (((t * steps).floor()) / (steps - 1.0).max(1.0)).clamp(0.0, 1.0)
                        }
                    }
                    StepPosition::JumpBoth => ((t * (steps + 1.0)).floor() / steps).clamp(0.0, 1.0),
                }
            }
        }
    }
}

/// Evaluates a cubic-bezier easing curve at progress `t` using Newton-Raphson.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    // Solve for the bezier parameter `u` such that x(u) == t, then return y(u).
    let bezier_axis = |a: f32, b: f32, u: f32| -> f32 {
        let inv = 1.0 - u;
        3.0 * inv * inv * u * a + 3.0 * inv * u * u * b + u * u * u
    };
    let bezier_axis_deriv = |a: f32, b: f32, u: f32| -> f32 {
        let inv = 1.0 - u;
        3.0 * inv * inv * a + 6.0 * inv * u * (b - a) + 3.0 * u * u * (1.0 - b)
    };

    let mut u = t;
    for _ in 0..8 {
        let x = bezier_axis(x1, x2, u) - t;
        if x.abs() < 1e-5 {
            break;
        }
        let d = bezier_axis_deriv(x1, x2, u);
        if d.abs() < 1e-6 {
            break;
        }
        u = (u - x / d).clamp(0.0, 1.0);
    }
    bezier_axis(y1, y2, u)
}

/// A CSS `transition` (one comma-separated entry of the `transition` shorthand).
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// `all`, or a property name such as `opacity` or `transform`.
    pub property: String,
    /// Duration in milliseconds.
    pub duration_ms: f32,
    pub timing: TimingFunction,
    /// Delay in milliseconds.
    pub delay_ms: f32,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            property: "all".to_string(),
            duration_ms: 0.0,
            timing: TimingFunction::Ease,
            delay_ms: 0.0,
        }
    }
}

/// CSS `animation-direction` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationDirection {
    #[default]
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/// CSS `animation-fill-mode` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationFillMode {
    #[default]
    None,
    Forwards,
    Backwards,
    Both,
}

/// CSS `animation-iteration-count` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationIterationCount {
    Finite(f32),
    Infinite,
}

impl Default for AnimationIterationCount {
    fn default() -> Self {
        AnimationIterationCount::Finite(1.0)
    }
}

/// CSS `animation-play-state` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationPlayState {
    #[default]
    Running,
    Paused,
}

/// A CSS `animation` declaration (one comma-separated entry of the shorthand).
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    /// Keyframes name (`@keyframes <name>`), or empty when unspecified.
    pub name: String,
    pub duration_ms: f32,
    pub timing: TimingFunction,
    pub delay_ms: f32,
    pub iteration_count: AnimationIterationCount,
    pub direction: AnimationDirection,
    pub fill_mode: AnimationFillMode,
    pub play_state: AnimationPlayState,
    pub timeline: Option<String>,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            name: String::new(),
            duration_ms: 0.0,
            timing: TimingFunction::Ease,
            delay_ms: 0.0,
            iteration_count: AnimationIterationCount::default(),
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::None,
            play_state: AnimationPlayState::Running,
            timeline: None,
        }
    }
}

/// Parses a CSS `<easing-function>` keyword or function call (e.g. `ease-in`, `steps(4, end)`).
pub fn parse_timing_function(s: &str) -> Option<TimingFunction> {
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "linear" => return Some(TimingFunction::Linear),
        "ease" => return Some(TimingFunction::Ease),
        "ease-in" => return Some(TimingFunction::EaseIn),
        "ease-out" => return Some(TimingFunction::EaseOut),
        "ease-in-out" => return Some(TimingFunction::EaseInOut),
        "step-start" => return Some(TimingFunction::Steps(1, StepPosition::JumpStart)),
        "step-end" => return Some(TimingFunction::Steps(1, StepPosition::JumpEnd)),
        _ => {}
    }
    if let Some(inner) = t
        .strip_prefix("cubic-bezier(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let nums: Vec<f32> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<f32>().ok())
            .collect();
        if nums.len() == 4 {
            return Some(TimingFunction::CubicBezier(
                nums[0], nums[1], nums[2], nums[3],
            ));
        }
        return None;
    }
    if let Some(inner) = t.strip_prefix("steps(").and_then(|s| s.strip_suffix(')')) {
        let mut parts = inner.split(',');
        let n = parts.next()?.trim().parse::<f32>().ok()?;
        let pos = match parts.next().map(|p| p.trim()) {
            Some("start") | Some("jump-start") => StepPosition::JumpStart,
            Some("jump-none") => StepPosition::JumpNone,
            Some("jump-both") => StepPosition::JumpBoth,
            _ => StepPosition::JumpEnd,
        };
        return Some(TimingFunction::Steps(n.max(1.0) as u32, pos));
    }
    None
}
