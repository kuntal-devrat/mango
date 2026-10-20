//! Media environment representation and CSS Media & Container Queries evaluation.

/// Color scheme preference for `prefers-color-scheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSchemePreference {
    #[default]
    Light,
    Dark,
}

/// Reduced motion preference for `prefers-reduced-motion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReducedMotionPreference {
    #[default]
    NoPreference,
    Reduce,
}

/// Contrast preference for `prefers-contrast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContrastPreference {
    #[default]
    NoPreference,
    More,
    Less,
    Custom,
}

/// Pointer accuracy type for `pointer` and `any-pointer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerType {
    #[default]
    Fine,
    Coarse,
    None,
}

/// Hover capability type for `hover` and `any-hover`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverType {
    #[default]
    Hover,
    None,
}

/// Represents the browser/client media environment for CSS Media Queries evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaEnvironment {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub color_scheme: ColorSchemePreference,
    pub reduced_motion: ReducedMotionPreference,
    pub contrast: ContrastPreference,
    pub hover: HoverType,
    pub any_hover: HoverType,
    pub pointer: PointerType,
    pub any_pointer: PointerType,
    pub device_pixel_ratio: f32,
}

impl Default for MediaEnvironment {
    fn default() -> Self {
        Self {
            viewport_width: 1280.0,
            viewport_height: 900.0,
            color_scheme: ColorSchemePreference::Light,
            reduced_motion: ReducedMotionPreference::NoPreference,
            contrast: ContrastPreference::NoPreference,
            hover: HoverType::Hover,
            any_hover: HoverType::Hover,
            pointer: PointerType::Fine,
            any_pointer: PointerType::Fine,
            device_pixel_ratio: 1.0,
        }
    }
}

impl MediaEnvironment {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            ..Default::default()
        }
    }

    pub fn is_landscape(&self) -> bool {
        self.viewport_width >= self.viewport_height
    }

    pub fn is_portrait(&self) -> bool {
        self.viewport_width < self.viewport_height
    }
}

thread_local! {
    static CURRENT_MEDIA_ENV: std::cell::RefCell<Option<MediaEnvironment>> = const { std::cell::RefCell::new(None) };
}

/// Returns the current media environment for the active thread.
pub fn current_media_environment() -> MediaEnvironment {
    CURRENT_MEDIA_ENV.with(|cell| cell.borrow().clone().unwrap_or_default())
}

/// Updates the media environment for the active thread.
pub fn set_media_environment(env: MediaEnvironment) {
    CURRENT_MEDIA_ENV.with(|cell| {
        *cell.borrow_mut() = Some(env);
    });
}

/// Sets the color scheme preference (`prefers-color-scheme: dark | light`).
pub fn set_prefers_color_scheme(scheme: ColorSchemePreference) {
    let mut env = current_media_environment();
    env.color_scheme = scheme;
    set_media_environment(env);
}

/// Sets the reduced motion preference (`prefers-reduced-motion: reduce | no-preference`).
pub fn set_prefers_reduced_motion(motion: ReducedMotionPreference) {
    let mut env = current_media_environment();
    env.reduced_motion = motion;
    set_media_environment(env);
}

/// Sets the contrast preference (`prefers-contrast: more | less | no-preference | custom`).
pub fn set_prefers_contrast(contrast: ContrastPreference) {
    let mut env = current_media_environment();
    env.contrast = contrast;
    set_media_environment(env);
}

/// Sets the device pixel ratio / resolution (1.0 = 1dppx / 96dpi).
pub fn set_device_pixel_ratio(dpr: f32) {
    let mut env = current_media_environment();
    env.device_pixel_ratio = dpr;
    set_media_environment(env);
}

/// Sets the pointer device capability (`pointer` and `any-pointer`).
pub fn set_pointer(pointer: PointerType) {
    let mut env = current_media_environment();
    env.pointer = pointer;
    env.any_pointer = pointer;
    set_media_environment(env);
}

/// Sets the hover capability (`hover` and `any-hover`).
pub fn set_hover(hover: HoverType) {
    let mut env = current_media_environment();
    env.hover = hover;
    env.any_hover = hover;
    set_media_environment(env);
}

/// Tests if a CSS media query matches the browser environment.
pub fn matches_media_query(query: &str, viewport_width: f32) -> bool {
    matches_media_query_size(query, viewport_width, 900.0)
}

/// Tests if a CSS media query matches the browser environment with explicit width and height.
pub fn matches_media_query_size(query: &str, viewport_width: f32, viewport_height: f32) -> bool {
    matches_media_query_env(
        query,
        viewport_width,
        viewport_height,
        &current_media_environment(),
    )
}

/// Tests if a CSS media query matches the specified media environment and viewport dimensions.
pub fn matches_media_query_env(
    query: &str,
    viewport_width: f32,
    viewport_height: f32,
    env: &MediaEnvironment,
) -> bool {
    matches_condition_internal(query, viewport_width, viewport_height, env, false)
}

/// Tests if a CSS container query condition matches given container dimensions.
pub fn matches_container_query(query: &str, container_width: f32, container_height: f32) -> bool {
    let env = current_media_environment();
    matches_condition_internal(query, container_width, container_height, &env, true)
}

pub(crate) fn matches_condition_internal(
    query: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    is_container: bool,
) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    for part in q.split(',') {
        let trimmed_part = part.trim();
        if trimmed_part.contains(" or ") {
            for subpart in trimmed_part.split(" or ") {
                if matches_single_condition(subpart.trim(), width, height, env, is_container) {
                    return true;
                }
            }
        } else if matches_single_condition(trimmed_part, width, height, env, is_container) {
            return true;
        }
    }
    false
}

fn matches_single_condition(
    query: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    is_container: bool,
) -> bool {
    let mut lower = query.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    if let Some(rest) = lower.strip_prefix("only ") {
        lower = rest.trim().to_string();
    }
    if let Some(rest) = lower.strip_prefix("not ") {
        return !matches_single_condition(rest.trim(), width, height, env, is_container);
    }

    while lower.starts_with('(') && lower.ends_with(')') {
        let mut depth = 0;
        let mut closed_early = false;
        for (i, c) in lower.char_indices() {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 && i < lower.len() - 1 {
                    closed_early = true;
                    break;
                }
            }
        }
        if depth == 0 && !closed_early {
            lower = lower[1..lower.len() - 1].trim().to_string();
        } else {
            break;
        }
    }

    let normalized = lower.replace(")and", ") and ").replace("and(", " and (");
    for condition in normalized.split(" and ") {
        let cond = condition.trim();
        if cond.is_empty() {
            continue;
        }

        if !is_container {
            if cond == "screen" || cond == "all" || cond == "handheld" {
                continue;
            }
            if cond == "print" || cond == "speech" {
                return false;
            }
        }

        let trimmed_cond = cond.trim();
        let inner = if trimmed_cond.starts_with('(')
            && trimmed_cond.ends_with(')')
            && trimmed_cond.len() >= 2
        {
            &trimmed_cond[1..trimmed_cond.len() - 1]
        } else {
            trimmed_cond
        }
        .trim();

        // 1. Check if it's a range condition (contains <=, >=, <, >, =)
        if let Some(res) = eval_range_condition(inner, width, height, env, is_container) {
            if !res {
                return false;
            }
            continue;
        }

        // 2. Feature: value condition
        if let Some((name, val_str)) = inner.split_once(':') {
            if !eval_feature_condition(
                name.trim(),
                val_str.trim(),
                width,
                height,
                env,
                is_container,
            ) {
                return false;
            }
            continue;
        }

        // 3. Boolean feature query: e.g. (hover), (pointer), (color)
        if !eval_boolean_feature(inner, env) {
            return false;
        }
    }

    true
}

fn eval_boolean_feature(feature: &str, env: &MediaEnvironment) -> bool {
    match feature {
        "hover" => env.hover == HoverType::Hover,
        "any-hover" => env.any_hover == HoverType::Hover,
        "pointer" => env.pointer != PointerType::None,
        "any-pointer" => env.any_pointer != PointerType::None,
        "color" => true,
        "all" | "screen" => true,
        _ => false,
    }
}

fn eval_feature_condition(
    feature: &str,
    val_str: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    _is_container: bool,
) -> bool {
    match feature {
        "min-width" | "min-inline-size" => {
            if let Some(val) = parse_media_length(val_str) {
                width >= val - 1e-4
            } else {
                false
            }
        }
        "max-width" | "max-inline-size" => {
            if let Some(val) = parse_media_length(val_str) {
                width <= val + 1e-4
            } else {
                false
            }
        }
        "width" | "inline-size" => {
            if let Some(val) = parse_media_length(val_str) {
                (width - val).abs() < 1.0
            } else {
                false
            }
        }
        "min-height" | "min-block-size" => {
            if let Some(val) = parse_media_length(val_str) {
                height >= val - 1e-4
            } else {
                false
            }
        }
        "max-height" | "max-block-size" => {
            if let Some(val) = parse_media_length(val_str) {
                height <= val + 1e-4
            } else {
                false
            }
        }
        "height" | "block-size" => {
            if let Some(val) = parse_media_length(val_str) {
                (height - val).abs() < 1.0
            } else {
                false
            }
        }
        "orientation" => {
            if val_str == "landscape" {
                width >= height
            } else if val_str == "portrait" {
                width < height
            } else {
                false
            }
        }
        "aspect-ratio" => {
            let ratio = width / height.max(1.0);
            parse_aspect_ratio(val_str).is_some_and(|t| (ratio - t).abs() < 1e-4)
        }
        "min-aspect-ratio" => {
            let ratio = width / height.max(1.0);
            parse_aspect_ratio(val_str).is_some_and(|t| ratio >= t - 1e-4)
        }
        "max-aspect-ratio" => {
            let ratio = width / height.max(1.0);
            parse_aspect_ratio(val_str).is_some_and(|t| ratio <= t + 1e-4)
        }
        "prefers-color-scheme" => {
            if val_str.contains("dark") {
                env.color_scheme == ColorSchemePreference::Dark
            } else if val_str.contains("light") {
                env.color_scheme == ColorSchemePreference::Light
            } else {
                true
            }
        }
        "prefers-reduced-motion" => {
            if val_str.contains("reduce") {
                env.reduced_motion == ReducedMotionPreference::Reduce
            } else {
                env.reduced_motion == ReducedMotionPreference::NoPreference
            }
        }
        "prefers-contrast" => match val_str {
            "more" => env.contrast == ContrastPreference::More,
            "less" => env.contrast == ContrastPreference::Less,
            "no-preference" => env.contrast == ContrastPreference::NoPreference,
            "custom" => env.contrast == ContrastPreference::Custom,
            _ => false,
        },
        "hover" => match val_str {
            "hover" => env.hover == HoverType::Hover,
            "none" => env.hover == HoverType::None,
            _ => false,
        },
        "any-hover" => match val_str {
            "hover" => env.any_hover == HoverType::Hover,
            "none" => env.any_hover == HoverType::None,
            _ => false,
        },
        "pointer" => match val_str {
            "fine" => env.pointer == PointerType::Fine,
            "coarse" => env.pointer == PointerType::Coarse,
            "none" => env.pointer == PointerType::None,
            _ => false,
        },
        "any-pointer" => match val_str {
            "fine" => env.any_pointer == PointerType::Fine,
            "coarse" => env.any_pointer == PointerType::Coarse,
            "none" => env.any_pointer == PointerType::None,
            _ => false,
        },
        "resolution" | "-webkit-device-pixel-ratio" => {
            parse_resolution(val_str).is_some_and(|t| (env.device_pixel_ratio - t).abs() < 1e-4)
        }
        "min-resolution" | "-webkit-min-device-pixel-ratio" => {
            parse_resolution(val_str).is_some_and(|t| env.device_pixel_ratio >= t - 1e-4)
        }
        "max-resolution" | "-webkit-max-device-pixel-ratio" => {
            parse_resolution(val_str).is_some_and(|t| env.device_pixel_ratio <= t + 1e-4)
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpOp {
    Le,
    Ge,
    Lt,
    Gt,
    Eq,
}

impl CmpOp {
    fn eval(self, a: f32, b: f32) -> bool {
        match self {
            CmpOp::Le => a <= b + 1e-4,
            CmpOp::Ge => a >= b - 1e-4,
            CmpOp::Lt => a < b - 1e-4,
            CmpOp::Gt => a > b + 1e-4,
            CmpOp::Eq => (a - b).abs() < 1e-4,
        }
    }
}

fn scan_operators(s: &str) -> Vec<(usize, usize, CmpOp)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut ops = Vec::new();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                ops.push((i, i + 2, CmpOp::Le));
                i += 2;
                continue;
            } else {
                ops.push((i, i + 1, CmpOp::Lt));
                i += 1;
                continue;
            }
        } else if bytes[i] == b'>' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                ops.push((i, i + 2, CmpOp::Ge));
                i += 2;
                continue;
            } else {
                ops.push((i, i + 1, CmpOp::Gt));
                i += 1;
                continue;
            }
        } else if bytes[i] == b'=' {
            ops.push((i, i + 1, CmpOp::Eq));
            i += 1;
            continue;
        }
        i += 1;
    }
    ops
}

fn resolve_range_operand(
    s: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    _is_container: bool,
) -> Option<f32> {
    let s_lower = s.trim().to_ascii_lowercase();
    match s_lower.as_str() {
        "width" | "inline-size" => Some(width),
        "height" | "block-size" => Some(height),
        "aspect-ratio" => Some(width / height.max(1.0)),
        "resolution" | "device-pixel-ratio" | "-webkit-device-pixel-ratio" => {
            Some(env.device_pixel_ratio)
        }
        _ => {
            if let Some(num_str) = s_lower.strip_suffix("vw")
                && let Ok(val) = num_str.trim().parse::<f32>()
            {
                return Some((val / 100.0) * width);
            }
            if let Some(num_str) = s_lower.strip_suffix("vh")
                && let Ok(val) = num_str.trim().parse::<f32>()
            {
                return Some((val / 100.0) * height);
            }
            if s_lower.contains('/') {
                let parts: Vec<&str> = s_lower.split('/').collect();
                if parts.len() == 2 {
                    let num = parts[0].trim().parse::<f32>().ok()?;
                    let den = parts[1].trim().parse::<f32>().ok()?;
                    return Some(num / den.max(1e-5));
                }
            }
            if let Some(len) = parse_media_length(s) {
                return Some(len);
            }
            if let Some(res) = parse_resolution(s) {
                return Some(res);
            }
            s.trim().parse::<f32>().ok()
        }
    }
}

fn eval_range_condition(
    inner: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    is_container: bool,
) -> Option<bool> {
    let ops = scan_operators(inner);
    if ops.is_empty() {
        return None;
    }
    if ops.len() == 1 {
        let (start, end, op) = ops[0];
        let lhs_str = inner[..start].trim();
        let rhs_str = inner[end..].trim();
        let v1 = resolve_range_operand(lhs_str, width, height, env, is_container)?;
        let v2 = resolve_range_operand(rhs_str, width, height, env, is_container)?;
        Some(op.eval(v1, v2))
    } else if ops.len() == 2 {
        let (start1, end1, op1) = ops[0];
        let (start2, end2, op2) = ops[1];
        let p1 = inner[..start1].trim();
        let p2 = inner[end1..start2].trim();
        let p3 = inner[end2..].trim();
        let v1 = resolve_range_operand(p1, width, height, env, is_container)?;
        let v2 = resolve_range_operand(p2, width, height, env, is_container)?;
        let v3 = resolve_range_operand(p3, width, height, env, is_container)?;
        Some(op1.eval(v1, v2) && op2.eval(v2, v3))
    } else {
        None
    }
}

fn parse_aspect_ratio(s: &str) -> Option<f32> {
    let s = s.trim();
    if s.contains('/') {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() == 2 {
            let num = parts[0].trim().parse::<f32>().ok()?;
            let den = parts[1].trim().parse::<f32>().ok()?;
            return Some(num / den.max(1e-5));
        }
    }
    s.parse::<f32>().ok()
}

fn parse_resolution(s: &str) -> Option<f32> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(num_str) = s.strip_suffix("dppx") {
        num_str.trim().parse::<f32>().ok()
    } else if let Some(num_str) = s.strip_suffix('x') {
        num_str.trim().parse::<f32>().ok()
    } else if let Some(num_str) = s.strip_suffix("dpi") {
        num_str.trim().parse::<f32>().ok().map(|dpi| dpi / 96.0)
    } else if let Some(num_str) = s.strip_suffix("dpcm") {
        num_str
            .trim()
            .parse::<f32>()
            .ok()
            .map(|dpcm| dpcm / 37.795_277)
    } else {
        s.parse::<f32>().ok()
    }
}

fn parse_media_length(s: &str) -> Option<f32> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(inner) = s.strip_prefix("calc(").and_then(|x| x.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split('-').collect();
        if parts.len() == 2 {
            let left = parse_simple_length(parts[0].trim())?;
            let right = parse_simple_length(parts[1].trim())?;
            return Some(left - right);
        }
        let parts_add: Vec<&str> = inner.split('+').collect();
        if parts_add.len() == 2 {
            let left = parse_simple_length(parts_add[0].trim())?;
            let right = parse_simple_length(parts_add[1].trim())?;
            return Some(left + right);
        }
        if let Some(val) = parse_simple_length(inner.trim()) {
            return Some(val);
        }
    }
    parse_simple_length(&s)
}

fn parse_simple_length(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(num_str) = s.strip_suffix("px") {
        num_str.trim().parse::<f32>().ok()
    } else if let Some(num_str) = s.strip_suffix("rem") {
        num_str.trim().parse::<f32>().ok().map(|rem| rem * 16.0)
    } else if let Some(num_str) = s.strip_suffix("em") {
        num_str.trim().parse::<f32>().ok().map(|em| em * 16.0)
    } else if let Some(num_str) = s.strip_suffix("pt") {
        num_str
            .trim()
            .parse::<f32>()
            .ok()
            .map(|pt| pt * (96.0 / 72.0))
    } else if let Some(num_str) = s.strip_suffix("in") {
        num_str.trim().parse::<f32>().ok().map(|i| i * 96.0)
    } else if let Some(num_str) = s.strip_suffix("cm") {
        num_str
            .trim()
            .parse::<f32>()
            .ok()
            .map(|cm| cm * (96.0 / 2.54))
    } else if let Some(num_str) = s.strip_suffix("mm") {
        num_str
            .trim()
            .parse::<f32>()
            .ok()
            .map(|mm| mm * (9.6 / 2.54))
    } else if let Some(num_str) = s.strip_suffix("ch") {
        num_str.trim().parse::<f32>().ok().map(|ch| ch * 8.0)
    } else if let Some(num_str) = s.strip_suffix("ex") {
        num_str.trim().parse::<f32>().ok().map(|ex| ex * 8.0)
    } else {
        s.parse::<f32>().ok()
    }
}
