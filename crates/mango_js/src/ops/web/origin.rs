use deno_core::OpState;

pub fn get_page_origin(state: &OpState) -> String {
    let page_url = state
        .try_borrow::<crate::ops::dom::PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_default();
    match mango_net::Url::parse(&page_url) {
        Ok(u) if u.host.is_some() => {
            let port_part = match (u.scheme.as_str(), u.port) {
                ("http", Some(80)) | ("https", Some(443)) => String::new(),
                (_, Some(p)) => format!(":{p}"),
                (_, None) => String::new(),
            };
            format!(
                "{}://{}{}",
                u.scheme.to_ascii_lowercase(),
                u.host.as_deref().unwrap_or("").to_ascii_lowercase(),
                port_part
            )
        }
        _ => page_url,
    }
}
