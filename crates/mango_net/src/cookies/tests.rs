//! Comprehensive RFC 6265 / RFC 6265bis Cookie tests.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::cookies::public_suffix::get_registrable_domain;
    use crate::cookies::*;
    use crate::url::Url;

    fn url(s: &str) -> Url {
        Url::parse(s).expect("url")
    }

    #[test]
    fn test_cookie_jar_storage_and_header_generation() {
        let mut jar = CookieJar::new();
        let u = url("https://duckduckgo.com/search?q=rust");

        let c1 = CookieJar::parse_set_cookie("session=abc123; Path=/; Secure; HttpOnly", &u)
            .expect("cookie 1");
        jar.store_cookie(c1);
        let c2 = CookieJar::parse_set_cookie("theme=dark; Path=/search; Max-Age=3600", &u)
            .expect("cookie 2");
        jar.store_cookie(c2);

        let header = jar.get_cookie_header(&u).expect("header");
        assert!(header.contains("session=abc123"));
        assert!(header.contains("theme=dark"));
        // http:// must not receive the Secure cookie.
        let plain = url("http://duckduckgo.com/search");
        let plain_header = jar.get_cookie_header(&plain).unwrap_or_default();
        assert!(!plain_header.contains("session=abc123"));
    }

    #[test]
    fn test_cookie_deletion_via_max_age_zero() {
        let mut jar = CookieJar::new();
        let u = url("https://duckduckgo.com/");
        jar.store_cookie(
            CookieJar::parse_set_cookie("p=1; Domain=duckduckgo.com; Path=/", &u).unwrap(),
        );
        assert_eq!(jar.len(), 1);
        jar.store_cookie(
            CookieJar::parse_set_cookie("p=deleted; Max-Age=0; Domain=duckduckgo.com; Path=/", &u)
                .unwrap(),
        );
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn test_default_path_follows_last_slash() {
        let u = url("https://example.com/a/b/c?x=1");
        let c = CookieJar::parse_set_cookie("k=v", &u).unwrap();
        assert_eq!(c.path, "/a/b");
        let root = url("https://example.com/");
        let rc = CookieJar::parse_set_cookie("k=v", &root).unwrap();
        assert_eq!(rc.path, "/");
    }

    #[test]
    fn test_host_only_cookie_is_not_sent_to_subdomains() {
        let mut jar = CookieJar::new();
        let host = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("a=1", &host).unwrap());
        let sub = url("https://api.example.com/");
        assert!(jar.get_cookie_header(&sub).is_none());

        let with_domain = CookieJar::parse_set_cookie("b=2; Domain=.example.com", &host).unwrap();
        jar.store_cookie(with_domain);
        assert!(jar.get_cookie_header(&sub).is_some());
    }

    #[test]
    fn test_http_only_cookies_are_hidden_from_scripts() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("secret=1; HttpOnly", &u).unwrap());
        jar.store_cookie(CookieJar::parse_set_cookie("open=2", &u).unwrap());
        let script_view = jar.script_cookie_string(&u);
        assert!(!script_view.contains("secret=1"));
        assert!(script_view.contains("open=2"));
    }

    #[test]
    fn test_document_cookie_assignment_roundtrip() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/app/page");
        assert!(jar.set_from_script("token=xyz; Path=/", &u));
        assert_eq!(jar.script_cookie_string(&u), "token=xyz");

        // HttpOnly must not be settable from a script.
        jar.clear();
        assert!(jar.set_from_script("evil=1; HttpOnly", &u));
        assert!(jar.script_cookie_string(&u).contains("evil=1"));
        assert!(jar.get_cookies(&u)[0].is_script_visible());
    }

    #[test]
    fn test_profile_persistence_roundtrip() {
        let mut jar = CookieJar::new();
        let u = url("https://news.ycombinator.com/item?id=1");
        jar.store_cookie(
            CookieJar::parse_set_cookie(
                "user=alice; Domain=news.ycombinator.com; Path=/; Max-Age=86400; SameSite=Lax; Secure",
                &u,
            )
            .unwrap(),
        );
        jar.store_cookie(CookieJar::parse_set_cookie("anon_session=1; Path=/item", &u).unwrap());

        let serialized = jar.to_profile_string();
        assert!(serialized.contains("news.ycombinator.com"));

        let mut restored = CookieJar::new();
        restored.load_profile_string(&serialized);
        assert_eq!(restored.len(), 2);

        let a = jar.all();
        let b = restored.all();
        assert_eq!(a.len(), b.len());
        for x in &a {
            let y = b
                .iter()
                .find(|c| c.name == x.name)
                .expect("cookie preserved");
            assert_eq!(x.value, y.value);
            assert_eq!(x.domain, y.domain);
            assert_eq!(x.path, y.path);
            assert_eq!(x.secure, y.secure);
            assert_eq!(x.http_only, y.http_only);
            assert_eq!(x.same_site, y.same_site);
            assert_eq!(x.host_only, y.host_only);
            assert_eq!(x.expires_at, y.expires_at);
        }
        // The Max-Age cookie survived as an absolute expiry timestamp.
        let with_expiry = b.iter().find(|c| c.name == "user").unwrap();
        assert!(with_expiry.expires_at.is_some());
    }

    #[test]
    fn test_profile_file_roundtrip_on_disk() {
        let dir = std::env::temp_dir().join(format!("mango_cookies_{}", std::process::id()));
        let path = dir.join("cookies.txt");
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("k=v; Max-Age=600", &u).unwrap());
        jar.save_to_file(&path).expect("save");

        let mut restored = CookieJar::new();
        let count = restored.load_from_file(&path).expect("load");
        assert_eq!(count, 1);
        assert_eq!(restored.get_cookie_header(&u).unwrap(), "k=v");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_expired_cookies_are_pruned_and_not_sent() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        let mut cookie = CookieJar::parse_set_cookie("old=1", &u).unwrap();
        cookie.expires_at = Some(1); // 1970-01-01
        jar.cookies.push(cookie);
        assert!(jar.get_cookie_header(&u).is_none());
        assert_eq!(jar.prune_expired(), 1);
        assert!(jar.is_empty());
    }

    #[test]
    fn test_expires_header_parsing() {
        // RFC 1123
        assert_eq!(
            parse_http_date("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1_445_412_480)
        );
        // RFC 850
        assert_eq!(
            parse_http_date("Wednesday, 21-Oct-15 07:28:00 GMT"),
            Some(1_445_412_480)
        );
        // asctime
        assert_eq!(
            parse_http_date("Wed Oct 21 07:28:00 2015"),
            Some(1_445_412_480)
        );
        assert_eq!(parse_http_date("not a date"), None);
    }

    #[test]
    fn test_https_expires_attribute_marks_cookie_deleted() {
        let u = url("https://example.com/");
        let c = CookieJar::parse_set_cookie(
            "sess=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/",
            &u,
        )
        .unwrap();
        assert_eq!(c.value, "");
        assert!(c.is_expired());
    }

    #[test]
    fn test_longer_paths_come_first_in_cookie_header() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/deep/nested/page");
        jar.store_cookie(CookieJar::parse_set_cookie("a=root; Path=/", &u).unwrap());
        jar.store_cookie(CookieJar::parse_set_cookie("b=deep; Path=/deep/nested", &u).unwrap());
        let header = jar.get_cookie_header(&u).unwrap();
        assert!(header.starts_with("b=deep"), "got {header}");
    }

    #[test]
    fn test_samesite_attribute_parsing() {
        let u = url("https://example.com/");
        assert_eq!(
            CookieJar::parse_set_cookie("a=1; SameSite=Strict", &u)
                .unwrap()
                .same_site,
            SameSite::Strict
        );
        assert_eq!(
            CookieJar::parse_set_cookie("a=1; SameSite=lax", &u)
                .unwrap()
                .same_site,
            SameSite::Lax
        );
        assert_eq!(
            CookieJar::parse_set_cookie("b=1", &u).unwrap().same_site,
            SameSite::Unspecified
        );
    }

    #[test]
    fn test_profile_line_escapes_control_characters() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        let mut cookie = CookieJar::parse_set_cookie("weird=v", &u).unwrap();
        cookie.value = "tab\there\nnext".to_string();
        jar.store_cookie(cookie);

        let body = jar.to_profile_string();
        assert_eq!(body.lines().count(), 3); // 2 comment lines + 1 cookie line

        let mut restored = CookieJar::new();
        restored.load_profile_string(&body);
        assert_eq!(restored.all()[0].value, "tab\there\nnext");
    }

    #[test]
    fn test_rfc6265_cross_domain_rejection() {
        let u = url("https://evil.com/");
        // Setting cookie for bank.com from evil.com must be rejected
        assert!(CookieJar::parse_set_cookie("auth=stolen; Domain=bank.com", &u).is_none());
        assert!(CookieJar::parse_set_cookie("auth=stolen; Domain=other.evil.com", &u).is_none());
    }

    #[test]
    fn test_rfc6265_public_suffix_rejection() {
        let u = url("https://example.com/");
        // Setting cookie for top-level domain .com must be rejected
        assert!(CookieJar::parse_set_cookie("tracker=1; Domain=com", &u).is_none());
        assert!(CookieJar::parse_set_cookie("tracker=1; Domain=.com", &u).is_none());

        let uk = url("https://shop.co.uk/");
        // Multi-part TLD .co.uk must be rejected
        assert!(CookieJar::parse_set_cookie("tracker=1; Domain=co.uk", &uk).is_none());

        // Cloud provider and PaaS shared domains must be rejected
        let heroku = url("https://myapp.herokuapp.com/");
        assert!(CookieJar::parse_set_cookie("shared=1; Domain=herokuapp.com", &heroku).is_none());
        assert_eq!(
            get_registrable_domain("myapp.herokuapp.com"),
            "myapp.herokuapp.com"
        );

        // 3-part public suffixes (e.g. s3.amazonaws.com) must be protected
        let s3 = url("https://mybucket.s3.amazonaws.com/");
        assert!(CookieJar::parse_set_cookie("shared=1; Domain=s3.amazonaws.com", &s3).is_none());
        assert_eq!(
            get_registrable_domain("mybucket.s3.amazonaws.com"),
            "mybucket.s3.amazonaws.com"
        );
    }

    #[test]
    fn test_rfc6265_ip_address_matching() {
        let u = url("https://192.168.1.100/");
        // IP address can only match exact host
        assert!(CookieJar::parse_set_cookie("k=v; Domain=1.100", &u).is_none());
        let c = CookieJar::parse_set_cookie("k=v; Domain=192.168.1.100", &u).unwrap();
        assert_eq!(c.domain, "192.168.1.100");

        let mut jar = CookieJar::new();
        jar.store_cookie(c);
        assert!(jar.get_cookie_header(&u).is_some());

        let other_ip = url("https://192.168.1.200/");
        assert!(jar.get_cookie_header(&other_ip).is_none());
    }

    #[test]
    fn test_rfc6265bis_cookie_prefixes() {
        let secure_url = url("https://example.com/");
        let insecure_url = url("http://example.com/");

        // __Secure- requires HTTPS and Secure attribute
        assert!(CookieJar::parse_set_cookie("__Secure-sess=1", &secure_url).is_none());
        assert!(CookieJar::parse_set_cookie("__Secure-sess=1; Secure", &insecure_url).is_none());
        assert!(CookieJar::parse_set_cookie("__Secure-sess=1; Secure", &secure_url).is_some());

        // __Host- requires HTTPS, Secure, Path=/, and NO Domain attribute
        assert!(
            CookieJar::parse_set_cookie("__Host-id=1; Secure; Path=/", &insecure_url).is_none()
        );
        assert!(CookieJar::parse_set_cookie("__Host-id=1; Path=/", &secure_url).is_none());
        assert!(
            CookieJar::parse_set_cookie("__Host-id=1; Secure; Path=/sub", &secure_url).is_none()
        );
        assert!(
            CookieJar::parse_set_cookie(
                "__Host-id=1; Secure; Domain=example.com; Path=/",
                &secure_url
            )
            .is_none()
        );
        assert!(CookieJar::parse_set_cookie("__Host-id=1; Secure; Path=/", &secure_url).is_some());
    }

    #[test]
    fn test_samesite_none_requires_secure() {
        let u = url("https://example.com/");
        assert!(CookieJar::parse_set_cookie("track=1; SameSite=None", &u).is_none());
        assert!(CookieJar::parse_set_cookie("track=1; SameSite=None; Secure", &u).is_some());
    }

    #[test]
    fn test_sorting_by_creation_time_on_equal_path_length() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/app");
        let mut c1 = CookieJar::parse_set_cookie("first=1; Path=/app", &u).unwrap();
        c1.creation_time = 100;
        let mut c2 = CookieJar::parse_set_cookie("second=2; Path=/app", &u).unwrap();
        c2.creation_time = 200;
        jar.store_cookie(c2);
        jar.store_cookie(c1);

        let header = jar.get_cookie_header(&u).unwrap();
        assert!(header.starts_with("first=1"), "got {header}");
    }

    #[test]
    fn test_quoted_attributes_and_positive_max_age() {
        let u = url("https://example.com/");
        let c = CookieJar::parse_set_cookie(
            r#"val="hello world"; Path="/"; Max-Age="+3600"; SameSite="Strict""#,
            &u,
        )
        .unwrap();
        assert_eq!(c.value, "hello world");
        assert_eq!(c.path, "/");
        assert_eq!(c.same_site, SameSite::Strict);
        assert!(c.expires_at.unwrap() > now_secs());
    }

    #[test]
    fn test_clear_session_cookies() {
        let mut jar = CookieJar::new();
        let u = url("https://example.com/");
        jar.store_cookie(CookieJar::parse_set_cookie("session=1", &u).unwrap());
        jar.store_cookie(CookieJar::parse_set_cookie("persist=2; Max-Age=3600", &u).unwrap());
        assert_eq!(jar.len(), 2);
        let dropped = jar.clear_session_cookies();
        assert_eq!(dropped, 1);
        assert_eq!(jar.len(), 1);
        assert_eq!(jar.all()[0].name, "persist");
    }

    #[test]
    fn test_max_age_overflow_protection() {
        let u = url("https://example.com/");
        let c =
            CookieJar::parse_set_cookie("big=1; Max-Age=9223372036854775807; Path=/", &u).unwrap();
        let exp = c.expires_at.unwrap();
        let max_expected = now_secs() + 400 * 86_400 + 5;
        assert!(exp <= max_expected);
        assert!(exp > now_secs());
    }
}
