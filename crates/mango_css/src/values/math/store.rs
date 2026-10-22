//! Thread-safe interned math store with O(1) deduplication and LRU eviction.

use std::collections::{HashMap, VecDeque};
use std::sync::{OnceLock, RwLock};

use super::expr::{MathExpr, MathId};
use crate::values::length::Length;

static MATH_STORE: OnceLock<RwLock<MathStoreInner>> = OnceLock::new();

/// Thread-safe interned-math-expression store with O(1) dedup and LRU eviction.
struct MathStoreInner {
    by_id: HashMap<u32, MathExpr>,
    by_expr: HashMap<MathExpr, u32>,
    /// LRU order: front is least-recently-used, back is most-recently-used.
    lru: VecDeque<u32>,
    next_id: u32,
}

impl MathStoreInner {
    fn new() -> Self {
        Self {
            by_id: HashMap::new(),
            by_expr: HashMap::new(),
            lru: VecDeque::new(),
            next_id: 0,
        }
    }

    /// Marks `id` as most-recently-used.
    fn touch(&mut self, id: u32) {
        if let Some(pos) = self.lru.iter().position(|&x| x == id) {
            self.lru.remove(pos);
        }
        self.lru.push_back(id);
    }
}

fn get_math_store() -> &'static RwLock<MathStoreInner> {
    MATH_STORE.get_or_init(|| RwLock::new(MathStoreInner::new()))
}

/// Interns a MathExpr into the thread-safe global store and returns its MathId,
/// deduplicating existing identical expressions in O(1) via a reverse map.
pub fn intern_math(expr: MathExpr) -> MathId {
    const MAX_MATH_STORE_SIZE: usize = 16_384;
    let store = get_math_store();
    let mut guard = store.write().unwrap_or_else(|e| e.into_inner());
    if let Some(&id) = guard.by_expr.get(&expr) {
        guard.touch(id);
        return MathId(id);
    }
    if guard.by_id.len() >= MAX_MATH_STORE_SIZE
        && let Some(evicted) = guard.lru.pop_front()
    {
        if let Some(old_expr) = guard.by_id.remove(&evicted) {
            guard.by_expr.remove(&old_expr);
        }
        eprintln!("mango_css: math expression store full; evicted least-recently-used entry");
    }
    let id = guard.next_id;
    guard.next_id = guard.next_id.wrapping_add(1);
    guard.by_id.insert(id, expr.clone());
    guard.by_expr.insert(expr, id);
    guard.lru.push_back(id);
    MathId(id)
}

/// Clears all interned math expressions (useful for test harnesses or memory recovery).
pub fn clear_math_store() {
    let store = get_math_store();
    let mut guard = store.write().unwrap_or_else(|e| e.into_inner());
    guard.by_id.clear();
    guard.by_expr.clear();
    guard.lru.clear();
    guard.next_id = 0;
}

/// Resolves an interned MathId to concrete pixels.
pub fn eval_math(
    id: MathId,
    font_size: f32,
    root_font_size: f32,
    container_size: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> f32 {
    let store = get_math_store();
    let guard = store.read().unwrap_or_else(|e| e.into_inner());
    if let Some(expr) = guard.by_id.get(&id.0) {
        expr.to_px_with_viewports(
            font_size,
            root_font_size,
            container_size,
            viewport_width,
            viewport_height,
        )
    } else {
        0.0
    }
}

/// Retrieves a clone of the interned MathExpr for a MathId.
pub fn get_math(id: MathId) -> Option<MathExpr> {
    let store = get_math_store();
    let guard = store.read().unwrap_or_else(|e| e.into_inner());
    guard.by_id.get(&id.0).cloned()
}

/// Creates a Length from a MathExpr, performing constant folding to Length::Px or CalcLength if possible.
pub fn make_math_length(expr: MathExpr) -> Length {
    let simplified = expr.simplify();
    if simplified.is_dimensionless_number() {
        Length::Math(intern_math(simplified))
    } else if let Some(calc) = simplified.try_into_calc_length() {
        Length::Calc(calc)
    } else if let Some(px) = simplified.try_fold_px() {
        Length::Px(px)
    } else {
        Length::Math(intern_math(simplified))
    }
}

thread_local! {
    static ANCHOR_NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn intern_anchor_name(name: &str) -> u32 {
    ANCHOR_NAMES.with(|names| {
        let mut list = names.borrow_mut();
        if let Some(pos) = list.iter().position(|n| n == name) {
            pos as u32
        } else {
            let id = list.len() as u32;
            list.push(name.to_string());
            id
        }
    })
}

pub fn get_interned_anchor_name(id: u32) -> Option<String> {
    ANCHOR_NAMES.with(|names| names.borrow().get(id as usize).cloned())
}
