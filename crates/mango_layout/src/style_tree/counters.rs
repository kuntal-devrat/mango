use std::collections::HashMap;

/// Context for tracking scoped CSS counters and quotation nesting depths during style tree construction.
#[derive(Debug, Clone, Default)]
pub struct CounterContext {
    counters: HashMap<String, Vec<i32>>,
    pub quote_depth: usize,
}

impl CounterContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> i32 {
        self.counters
            .get(name)
            .and_then(|stack| stack.last().copied())
            .unwrap_or(0)
    }

    pub fn get_all(&self, name: &str) -> Vec<i32> {
        self.counters.get(name).cloned().unwrap_or_else(|| vec![0])
    }

    pub fn reset(&mut self, name: &str, value: i32) {
        self.counters
            .entry(name.to_string())
            .or_default()
            .push(value);
    }

    pub fn increment(&mut self, name: &str, value: i32) {
        let stack = self.counters.entry(name.to_string()).or_default();
        if let Some(top) = stack.last_mut() {
            *top += value;
        } else {
            stack.push(value);
        }
    }

    pub fn pop(&mut self, name: &str) {
        if let Some(stack) = self.counters.get_mut(name) {
            stack.pop();
        }
    }
}

pub fn format_counter_value(val: i32, style: &str) -> String {
    match style {
        "decimal" => val.to_string(),
        "decimal-leading-zero" => {
            if (0..10).contains(&val) {
                format!("0{}", val)
            } else if val < 0 && val > -10 {
                format!("-0{}", -val)
            } else {
                val.to_string()
            }
        }
        "none" => String::new(),
        "lower-alpha" | "lower-latin" => {
            if val <= 0 {
                return val.to_string();
            }
            int_to_alpha(val as u32, false)
        }
        "upper-alpha" | "upper-latin" => {
            if val <= 0 {
                return val.to_string();
            }
            int_to_alpha(val as u32, true)
        }
        "lower-roman" => {
            if val <= 0 || val > 3999 {
                return val.to_string();
            }
            int_to_roman(val as u32).to_ascii_lowercase()
        }
        "upper-roman" => {
            if val <= 0 || val > 3999 {
                return val.to_string();
            }
            int_to_roman(val as u32)
        }
        _ => val.to_string(),
    }
}

pub fn int_to_alpha(mut n: u32, upper: bool) -> String {
    let mut res = Vec::new();
    while n > 0 {
        n -= 1;
        let rem = (n % 26) as u8;
        let base = if upper { b'A' } else { b'a' };
        res.push((base + rem) as char);
        n /= 26;
    }
    res.reverse();
    res.into_iter().collect()
}

pub fn int_to_roman(mut n: u32) -> String {
    const ROMANS: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for &(val, name) in ROMANS {
        while n >= val {
            s.push_str(name);
            n -= val;
        }
    }
    s
}
