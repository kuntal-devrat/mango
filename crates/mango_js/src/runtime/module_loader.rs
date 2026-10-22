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
    cache: Arc<Mutex<HashMap<String, String>>>,
    resource_cache: Option<Arc<mango_net::ResourceCache>>,
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
                std::time::Duration::from_millis(500),
            )),
            cache: Arc::new(Mutex::new(HashMap::new())),
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
            let url_str = spec.to_string();
            // 1. Check local module loader memory cache
            if let Ok(guard) = self.cache.lock() {
                if let Some(code) = guard.get(&url_str) {
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

            // 3. Fallback: NEVER block the UI loop on synchronous network I/O.
            // Dispatch a background worker fetch to warm the cache for subsequent ticks,
            // and return a non-blocking proxy stub so V8 never starves the compositor heartbeat.
            log::warn!(
                "Module {url_str} not in memory cache on UI thread; pre-fetching on background worker"
            );
            let client = self.client.clone();
            let cache_store = self.cache.clone();
            let rcache_opt = self.resource_cache.clone();
            let target_url = url_str.clone();
            let _ = std::thread::Builder::new()
                .name("mango-bg-module-fetch".to_string())
                .spawn(move || {
                    if let Ok(u) = mango_net::Url::parse(&target_url) {
                        if let Ok(resp) = client.fetch(&u) {
                            let c = String::from_utf8_lossy(&resp.body).to_string();
                            if let Ok(mut g) = cache_store.lock() {
                                g.insert(target_url.clone(), c);
                            }
                            if let Some(ref rc) = rcache_opt {
                                rc.insert(
                                    &target_url,
                                    "application/javascript",
                                    resp.status,
                                    resp.headers,
                                    resp.body,
                                    None,
                                );
                            }
                        }
                    }
                });

            let stub_code = "const _p = new Proxy({}, { get: () => _p, apply: () => _p }); export default _p; export const __esModule = true;";
            ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(stub_code.to_string().into()),
                &spec,
                None,
            )))
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
