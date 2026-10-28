use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use deno_core::error::ModuleLoaderError;
use deno_core::{
    ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
    ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind, resolve_import,
};

/// ES Module loader supporting HTTP, HTTPS, import maps, and internal Mango module schemes.
pub struct MangoModuleLoader {
    page_url: String,
    import_maps: Arc<Mutex<HashMap<String, String>>>,
    client: Arc<mango_net::HttpClient>,
    cache: Arc<Mutex<ModuleCache>>,
    module_loads: Arc<std::sync::atomic::AtomicUsize>,
    resource_cache: Option<Arc<mango_net::ResourceCache>>,
}

/// Page-local module source cache. Module graphs on modern sites are large;
/// keeping every response forever turns one navigation into an unbounded heap
/// growth event. This cache is deliberately byte-bounded and deduplicates
/// concurrent fallback fetches.
struct ModuleCache {
    entries: HashMap<String, String>,
    bytes: usize,
}

impl ModuleCache {
    const MAX_BYTES: usize = 32 * 1024 * 1024;

    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
        }
    }

    fn insert(&mut self, key: String, value: String) {
        let size = value.len();
        if size > Self::MAX_BYTES {
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old.len());
        }
        while self.bytes + size > Self::MAX_BYTES {
            let Some(old_key) = self.entries.keys().next().cloned() else {
                break;
            };
            if let Some(old) = self.entries.remove(&old_key) {
                self.bytes = self.bytes.saturating_sub(old.len());
            }
        }
        self.bytes += size;
        self.entries.insert(key, value);
    }
}

impl MangoModuleLoader {
    /// Creates a new module loader bound to the given page URL and shared import map.
    pub fn new(page_url: &str, import_maps: Arc<Mutex<HashMap<String, String>>>) -> Self {
        Self::with_resource_cache(page_url, import_maps, None)
    }

    /// Creates a module loader with access to the preloaded background resource cache.
    pub fn with_resource_cache(
        page_url: &str,
        import_maps: Arc<Mutex<HashMap<String, String>>>,
        resource_cache: Option<Arc<mango_net::ResourceCache>>,
    ) -> Self {
        Self {
            page_url: page_url.to_string(),
            import_maps,
            client: Arc::new(mango_net::HttpClient::with_timeout(
                std::time::Duration::from_secs(30),
            )),
            cache: Arc::new(Mutex::new(ModuleCache::new())),
            module_loads: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            resource_cache,
        }
    }
}

impl ModuleLoader for MangoModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        log::info!("MangoModuleLoader::resolve: specifier='{specifier}', referrer='{referrer}'");
        let mapped_specifier = if let Ok(maps) = self.import_maps.lock() {
            if let Some(target) = maps.get(specifier) {
                target.clone()
            } else {
                let mut prefix_match = None;
                for (k, v) in maps.iter() {
                    if k.ends_with('/') && specifier.starts_with(k) {
                        prefix_match = Some(format!("{}{}", v, &specifier[k.len()..]));
                        break;
                    }
                }
                prefix_match.unwrap_or_else(|| specifier.to_string())
            }
        } else {
            specifier.to_string()
        };

        let ref_url = if referrer.is_empty() || referrer == "." || referrer == "about:blank" {
            &self.page_url
        } else {
            referrer
        };

        match resolve_import(&mapped_specifier, ref_url) {
            Ok(spec) => Ok(spec),
            Err(e) => {
                if !mapped_specifier.starts_with('/')
                    && !mapped_specifier.starts_with("./")
                    && !mapped_specifier.starts_with("../")
                    && !mapped_specifier.starts_with("http://")
                    && !mapped_specifier.starts_with("https://")
                {
                    let clean_id: String = mapped_specifier
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect();
                    if let Ok(stub_url) =
                        ModuleSpecifier::parse(&format!("mango://stub/{clean_id}.js"))
                    {
                        return Ok(stub_url);
                    }
                }
                Err(ModuleLoaderError::from_err(e))
            }
        }
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        log::info!("MangoModuleLoader::load: spec='{module_specifier}'");
        let spec = module_specifier.clone();
        if spec.scheme() == "http" || spec.scheme() == "https" {
            self.module_loads
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let url_str = spec.to_string();
            // 1. Check local module loader memory cache
            if let Ok(guard) = self.cache.lock() {
                if let Some(code) = guard.entries.get(&url_str) {
                    return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                        ModuleType::JavaScript,
                        ModuleSourceCode::String(code.clone().into()),
                        &spec,
                        None,
                    )));
                }
            }

            // 2. Check network resource cache pre-cached by background pipeline
            if let Some(ref rcache) = self.resource_cache {
                if let Some(cached) = rcache.get(&url_str) {
                    let code = String::from_utf8_lossy(&cached.body).to_string();
                    if let Ok(mut guard) = self.cache.lock() {
                        guard.insert(url_str.clone(), code.clone());
                    }
                    return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                        ModuleType::JavaScript,
                        ModuleSourceCode::String(code.into()),
                        &spec,
                        None,
                    )));
                }
            }

            // 3. Fallback: Fetch missing module resource directly so ES module resolution succeeds
            if let Ok(u) = mango_net::Url::parse(&url_str) {
                if let Ok(resp) = self.client.fetch(&u) {
                    if resp.status >= 200 && resp.status < 400 {
                        let code = String::from_utf8_lossy(&resp.body).to_string();
                        if let Ok(mut g) = self.cache.lock() {
                            g.insert(url_str.clone(), code.clone());
                        }
                        if let Some(ref rc) = self.resource_cache {
                            rc.insert(
                                &url_str,
                                "application/javascript",
                                resp.status,
                                resp.headers,
                                resp.body,
                                None,
                            );
                        }
                        return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(code.into()),
                            &spec,
                            None,
                        )));
                    }
                }
            }

            ModuleLoadResponse::Sync(Err(ModuleLoaderError::generic(format!(
                "Failed to fetch module resource: {url_str}"
            ))))
        } else if spec.scheme() == "mango" {
            // Internal synthetic/empty stub module with Proxy exports
            let stub_code = "const _p = new Proxy({}, { get: () => _p, apply: () => _p }); export default _p; export const __esModule = true;";
            ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(stub_code.to_string().into()),
                &spec,
                None,
            )))
        } else {
            ModuleLoadResponse::Sync(Err(ModuleLoaderError::generic(format!(
                "Unsupported module specifier scheme: {}",
                spec.scheme()
            ))))
        }
    }
}
