use super::types::SharedLocalStorage;

/// Escapes a profile-field value so one entry always occupies one line.
pub fn escape_profile_field(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// Unescapes a profile-field value.
pub fn unescape_profile_field(s: &str) -> String {
    s.replace("%0D", "\r")
        .replace("%0A", "\n")
        .replace("%09", "\t")
        .replace("%25", "%")
}

/// Restores a `localStorage` profile (`<origin>\t<key>\t<value>` per line) into the store.
pub fn load_local_storage_from_file(
    path: &std::path::Path,
    store: &SharedLocalStorage,
) -> std::io::Result<usize> {
    let body = std::fs::read_to_string(path)?;
    let mut count = 0usize;
    if let Ok(mut map) = store.lock() {
        for line in body.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let origin = unescape_profile_field(parts[0]);
                let k = unescape_profile_field(parts[1]);
                let value = unescape_profile_field(parts[2]);
                map.insert(format!("{origin}\x1f{k}"), value);
                count += 1;
            } else if parts.len() == 2 {
                map.insert(
                    unescape_profile_field(parts[0]),
                    unescape_profile_field(parts[1]),
                );
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Writes the `localStorage` store to its profile file.
pub fn save_local_storage_to_file(
    path: &std::path::Path,
    store: &SharedLocalStorage,
) -> std::io::Result<()> {
    let map = match store.lock() {
        Ok(m) => m,
        Err(_) => {
            return Err(std::io::Error::other("Lock poisoned"));
        }
    };
    let mut body = String::from("# Mango localStorage v1\n");
    for (key, value) in map.iter() {
        if let Some((origin, k)) = key.split_once('\x1f') {
            body.push_str(&format!(
                "{}\t{}\t{}\n",
                escape_profile_field(origin),
                escape_profile_field(k),
                escape_profile_field(value)
            ));
        } else {
            body.push_str(&format!(
                "{}\t{}\n",
                escape_profile_field(key),
                escape_profile_field(value)
            ));
        }
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, body)?;
    if std::fs::rename(&tmp_path, path).is_err() {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp_path, path)?;
    }
    Ok(())
}
