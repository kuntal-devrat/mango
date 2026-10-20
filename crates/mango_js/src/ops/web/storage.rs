use deno_core::{OpState, op2};

use crate::ops::web::origin::get_page_origin;
use crate::web_apis::{SharedLocalStorage, SharedSessionStorage};

#[derive(Clone)]
pub struct OpLocalStorage(pub SharedLocalStorage);

#[derive(Clone)]
pub struct OpSessionStorage(pub SharedSessionStorage);

// ── LocalStorage ─────────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_ls_get(state: &mut OpState, #[string] key: String) -> Option<String> {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    let store = state.try_borrow::<OpLocalStorage>()?;
    let map = store.0.lock().ok()?;
    map.get(&scoped_key).cloned()
}

#[op2(fast)]
pub fn op_mango_ls_set(state: &mut OpState, #[string] key: String, #[string] val: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.insert(scoped_key, val);
    }
}

#[op2(fast)]
pub fn op_mango_ls_remove(state: &mut OpState, #[string] key: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.remove(&scoped_key);
    }
}

#[op2(fast)]
pub fn op_mango_ls_clear(state: &mut OpState) {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.retain(|k, _| !k.starts_with(&prefix));
    }
}

#[op2]
#[serde]
pub fn op_mango_ls_keys(state: &mut OpState) -> Vec<String> {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(map) = store.0.lock()
    {
        map.keys()
            .filter(|k| k.starts_with(&prefix) && !k[prefix.len()..].starts_with("__idb__"))
            .map(|k| k[prefix.len()..].to_string())
            .collect()
    } else {
        Vec::new()
    }
}

// ── SessionStorage ───────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_ss_get(state: &mut OpState, #[string] key: String) -> Option<String> {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    let store = state.try_borrow::<OpSessionStorage>()?;
    let map = store.0.lock().ok()?;
    map.get(&scoped_key).cloned()
}

#[op2(fast)]
pub fn op_mango_ss_set(state: &mut OpState, #[string] key: String, #[string] val: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.insert(scoped_key, val);
    }
}

#[op2(fast)]
pub fn op_mango_ss_remove(state: &mut OpState, #[string] key: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.remove(&scoped_key);
    }
}

#[op2(fast)]
pub fn op_mango_ss_clear(state: &mut OpState) {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.retain(|k, _| !k.starts_with(&prefix));
    }
}

#[op2]
#[serde]
pub fn op_mango_ss_keys(state: &mut OpState) -> Vec<String> {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(map) = store.0.lock()
    {
        map.keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| k[prefix.len()..].to_string())
            .collect()
    } else {
        Vec::new()
    }
}

// ── IndexedDB Stubs ──────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_idb_load(state: &mut OpState, #[string] key: String) -> Option<String> {
    let orig = get_page_origin(state);
    let idb_key = format!("{orig}\x1f__idb__{key}");
    let store = state.try_borrow::<OpLocalStorage>()?;
    let map = store.0.lock().ok()?;
    map.get(&idb_key).cloned()
}

#[op2(fast)]
pub fn op_mango_idb_save(state: &mut OpState, #[string] key: String, #[string] val: String) {
    let orig = get_page_origin(state);
    let idb_key = format!("{orig}\x1f__idb__{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.insert(idb_key, val);
    }
}

#[op2(fast)]
pub fn op_mango_idb_delete(state: &mut OpState, #[string] key: String) {
    let orig = get_page_origin(state);
    let idb_key = format!("{orig}\x1f__idb__{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.remove(&idb_key);
    }
}

#[op2]
#[serde]
pub fn op_mango_idb_list(state: &mut OpState) -> Vec<String> {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f__idb__");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(map) = store.0.lock()
    {
        map.keys()
            .filter_map(|k| k.strip_prefix(&prefix).map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}
