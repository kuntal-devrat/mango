//! Public suffix and registrable domain determination (RFC 6265 §5.3 / RFC 6265bis).

use std::net::IpAddr;

/// Returns true if host represents an IPv4 or IPv6 address.
pub fn is_ip_address(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
}

/// Returns true if `domain` is a known public suffix (top-level or two-level domain).
/// RFC 6265 §5.3 step 5 forbids setting cookies for public suffixes.
pub fn is_public_suffix(domain: &str) -> bool {
    let d = domain.trim_start_matches('.').to_ascii_lowercase();
    if d.is_empty() {
        return false;
    }
    if !d.contains('.') {
        return d != "localhost";
    }

    const MULTI_PART_SUFFIXES: &[&str] = &[
        "co.uk",
        "org.uk",
        "gov.uk",
        "ac.uk",
        "net.uk",
        "me.uk",
        "ltd.uk",
        "plc.uk",
        "com.au",
        "net.au",
        "org.au",
        "edu.au",
        "gov.au",
        "asn.au",
        "id.au",
        "co.jp",
        "ne.jp",
        "or.jp",
        "ac.jp",
        "go.jp",
        "ed.jp",
        "co.nz",
        "org.nz",
        "net.nz",
        "govt.nz",
        "ac.nz",
        "geek.nz",
        "com.br",
        "org.br",
        "net.br",
        "gov.br",
        "edu.br",
        "com.cn",
        "net.cn",
        "org.cn",
        "gov.cn",
        "edu.cn",
        "co.in",
        "net.in",
        "org.in",
        "gen.in",
        "firm.in",
        "ind.in",
        "nic.in",
        "ac.in",
        "edu.in",
        "res.in",
        "gov.in",
        "mil.in",
        "co.za",
        "org.za",
        "gov.za",
        "ac.za",
        "net.za",
        "com.sg",
        "edu.sg",
        "gov.sg",
        "net.sg",
        "org.sg",
        "com.mx",
        "org.mx",
        "gob.mx",
        "edu.mx",
        "net.mx",
        "co.kr",
        "ne.kr",
        "or.kr",
        "re.kr",
        "pe.kr",
        "go.kr",
        "ac.kr",
        "com.tw",
        "org.tw",
        "net.tw",
        "gov.tw",
        "edu.tw",
        "idv.tw",
        "com.hk",
        "org.hk",
        "net.hk",
        "gov.hk",
        "edu.hk",
        "com.tr",
        "org.tr",
        "net.tr",
        "gov.tr",
        "edu.tr",
        "com.ua",
        "net.ua",
        "org.ua",
        "gov.ua",
        "edu.ua",
        "co.id",
        "or.id",
        "web.id",
        "go.id",
        "ac.id",
        "sch.id",
        "com.my",
        "org.my",
        "net.my",
        "gov.my",
        "edu.my",
        "com.ph",
        "org.ph",
        "net.ph",
        "gov.ph",
        "edu.ph",
        "com.vn",
        "net.vn",
        "org.vn",
        "gov.vn",
        "edu.vn",
        "com.ar",
        "org.ar",
        "gob.ar",
        "net.ar",
        "edu.ar",
        "co.il",
        "org.il",
        "gov.il",
        "ac.il",
        "muni.il",
        // Additional ccTLDs
        "com.es",
        "org.es",
        "gob.es",
        "edu.es",
        "nom.es",
        "asso.fr",
        "nom.fr",
        "com.fr",
        "co.it",
        "gov.it",
        "edu.it",
        "co.nl",
        "net.nl",
        "com.pl",
        "net.pl",
        "org.pl",
        "edu.pl",
        "gov.pl",
        "com.ru",
        "net.ru",
        "org.ru",
        "gov.ru",
        "edu.ru",
        "com.se",
        "org.se",
        "co.at",
        "or.at",
        "gv.at",
        "co.ch",
        "co.be",
        "com.pt",
        "org.pt",
        "gov.pt",
        "com.gr",
        "edu.gr",
        "com.ro",
        "org.ro",
        "com.co",
        "org.co",
        "edu.co",
        "gov.co",
        "com.cl",
        "com.pe",
        "edu.pe",
        "gob.pe",
        "com.ve",
        "com.ec",
        "co.th",
        "ac.th",
        "go.th",
        "com.pk",
        "org.pk",
        "com.bd",
        "gov.bd",
        "com.ng",
        "gov.ng",
        "com.eg",
        "gov.eg",
        "co.ke",
        // Major cloud platforms, shared hosting, and PaaS providers (RFC 6265bis cookie boundary protection)
        "github.io",
        "gitlab.io",
        "vercel.app",
        "netlify.app",
        "pages.dev",
        "workers.dev",
        "herokuapp.com",
        "herokuapp.net",
        "azurewebsites.net",
        "azurewebsites.windows.net",
        "cloudapp.net",
        "appspot.com",
        "firebaseapp.com",
        "web.app",
        "cloudfront.net",
        "s3.amazonaws.com",
        "s3-website.amazonaws.com",
        "elasticbeanstalk.com",
        "fly.dev",
        "railway.app",
        "onrender.com",
        "glitch.me",
        "replit.app",
        "replit.dev",
        "repl.co",
        "supabase.co",
        "deno.dev",
        "ngrok.io",
        "ngrok-free.app",
        "fastly.net",
        "fastlylb.net",
        "myshopify.com",
        "blogspot.com",
        "wordpress.com",
        "wixsite.com",
        "azureedge.net",
        "trafficmanager.net",
        "surge.sh",
    ];

    MULTI_PART_SUFFIXES.iter().any(|&s| d == s)
}

/// Returns the registrable domain (eTLD+1) for a hostname according to RFC 6265bis.
pub fn get_registrable_domain(host: &str) -> String {
    let d = host.trim().trim_start_matches('.').to_ascii_lowercase();
    if is_ip_address(&d) || d == "localhost" {
        return d;
    }
    let parts: Vec<&str> = d.split('.').collect();
    if parts.len() <= 2 {
        return d;
    }

    // Check for 3-part public suffixes (e.g. s3.amazonaws.com, azurewebsites.windows.net)
    if parts.len() >= 4 {
        let last_three = format!(
            "{}.{}.{}",
            parts[parts.len() - 3],
            parts[parts.len() - 2],
            parts[parts.len() - 1]
        );
        if is_public_suffix(&last_three) {
            return format!("{}.{}", parts[parts.len() - 4], last_three);
        }
    }

    // Check for 2-part public suffixes (e.g. co.uk, com.au, github.io)
    let last_two = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    if is_public_suffix(&last_two) && parts.len() >= 3 {
        return format!("{}.{}", parts[parts.len() - 3], last_two);
    }
    format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
}

/// Checks if two hostnames belong to the same site (same registrable domain).
pub fn is_same_site_hosts(h1: &str, h2: &str) -> bool {
    let d1 = get_registrable_domain(h1);
    let d2 = get_registrable_domain(h2);
    d1 == d2
}
