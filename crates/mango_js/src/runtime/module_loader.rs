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
}

impl MangoModuleLoader {
    /// Creates a new module loader bound to the given page URL and shared import map.
    pub fn new(page_url: &str, import_maps: Arc<Mutex<HashMap<String, String>>>) -> Self {
        Self {
            page_url: page_url.to_string(),
            import_maps,
            client: Arc::new(mango_net::HttpClient::new()),
            cache: Arc::new(Mutex::new(HashMap::new())),
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
            if let Ok(url) = mango_net::Url::parse(&url_str) {
                match self.client.fetch(&url) {
                    Ok(resp) => {
                        let code = String::from_utf8_lossy(&resp.body).to_string();
                        if let Ok(mut guard) = self.cache.lock() {
                            guard.insert(url_str, code.clone());
                        }
                        return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(code.into()),
                            &spec,
                            None,
                        )));
                    }
                    Err(e) => {
                        return ModuleLoadResponse::Sync(Err(ModuleLoaderError::generic(format!(
                            "Failed to fetch module {url_str}: {e}"
                        ))));
                    }
                }
            }
            ModuleLoadResponse::Sync(Err(ModuleLoaderError::generic(format!(
                "Invalid URL: {url_str}"
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
