use crate::types::Page;

/// Normalize a user-typed inspect target to an http(s) URL.
///
/// This is for the debugger search bar only. Do not run it on strings that
/// came from a page payload.
pub fn normalize_inspect_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("enter a site url".to_string());
    }
    if trimmed
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err("url must not contain spaces".to_string());
    }

    let lower = trimmed.to_ascii_lowercase();
    const BLOCKED: &[&str] = &[
        "javascript:",
        "data:",
        "file:",
        "vbscript:",
        "blob:",
        "chrome:",
        "chrome-extension:",
        "about:",
        "view-source:",
        "ws:",
        "wss:",
        "ftp:",
    ];
    if BLOCKED.iter().any(|scheme| lower.starts_with(scheme)) {
        return Err("only http and https urls".to_string());
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else if looks_local(trimmed) {
        format!("http://{trimmed}")
    } else {
        format!("https://{trimmed}")
    };

    let (scheme, rest) = candidate
        .split_once("://")
        .ok_or_else(|| "invalid url".to_string())?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("only http and https urls".to_string());
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if !authority_is_ok(authority) {
        return Err("url needs a host".to_string());
    }
    Ok(candidate)
}

/// Scheme + host + port, with no path. `None` if the URL is not http(s).
pub fn origin_from_http_url(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if !authority_is_ok(authority) {
        return None;
    }
    Some(format!("{scheme}://{authority}"))
}

pub fn page_matches_inspect_url(page: &Page, url: &str) -> bool {
    if page.url == url {
        return true;
    }
    let Some(want) = origin_from_http_url(url) else {
        return false;
    };
    if page.origin == want {
        return true;
    }
    origin_from_http_url(&page.url).as_deref() == Some(want.as_str())
}

pub fn find_page_for_url<'a>(pages: &'a [Page], url: &str) -> Option<&'a Page> {
    pages.iter().find(|page| page.url == url).or_else(|| {
        pages
            .iter()
            .find(|page| page_matches_inspect_url(page, url))
    })
}

fn looks_local(raw: &str) -> bool {
    let host = host_of(raw);
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host.eq_ignore_ascii_case("[::1]")
}

fn host_of(raw: &str) -> &str {
    let authority = raw.split(['/', '?', '#']).next().unwrap_or(raw);
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    strip_port(hostport)
}

fn strip_port(hostport: &str) -> &str {
    if let Some(end) = hostport.find(']') {
        return &hostport[..=end];
    }
    if let Some((host, port)) = hostport.rsplit_once(':') {
        if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return host;
        }
    }
    hostport
}

fn authority_is_ok(authority: &str) -> bool {
    if authority.is_empty() || authority.contains([' ', '\\', '/']) {
        return false;
    }
    let hostport = authority.rsplit('@').next().unwrap_or("");
    if hostport.is_empty() {
        return false;
    }
    let host = strip_port(hostport);
    if host.is_empty() || host == "." || host.starts_with('.') || host.ends_with('.') {
        return false;
    }
    if host.starts_with('[') {
        return host.ends_with(']') && host.len() > 2;
    }
    if let Some((_, port)) = hostport.rsplit_once(':') {
        if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::PageId;

    fn page(url: &str, origin: &str) -> Page {
        Page {
            id: PageId::from("tab:1"),
            url: url.to_string(),
            title: "demo".to_string(),
            origin: origin.to_string(),
        }
    }

    #[test]
    fn adds_https_for_public_hosts() {
        assert_eq!(
            normalize_inspect_url("example.com").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn uses_http_for_localhost_and_loopback() {
        assert_eq!(
            normalize_inspect_url("localhost:5173").unwrap(),
            "http://localhost:5173"
        );
        assert_eq!(
            normalize_inspect_url("127.0.0.1:5173").unwrap(),
            "http://127.0.0.1:5173"
        );
    }

    #[test]
    fn keeps_explicit_http() {
        assert_eq!(
            normalize_inspect_url("http://localhost:5173/").unwrap(),
            "http://localhost:5173/"
        );
    }

    #[test]
    fn rejects_dangerous_schemes() {
        assert!(normalize_inspect_url("javascript:alert(1)").is_err());
        assert!(normalize_inspect_url("file:///etc/passwd").is_err());
        assert!(normalize_inspect_url("data:text/html,hi").is_err());
        assert!(normalize_inspect_url("chrome://flags").is_err());
    }

    #[test]
    fn rejects_empty_and_spaces() {
        assert!(normalize_inspect_url("").is_err());
        assert!(normalize_inspect_url("https://exa mple.com").is_err());
    }

    #[test]
    fn origin_strips_path() {
        assert_eq!(
            origin_from_http_url("http://localhost:5173/app").as_deref(),
            Some("http://localhost:5173")
        );
    }

    #[test]
    fn matches_page_by_origin_when_path_differs() {
        let listed = page("http://localhost:5173/", "http://localhost:5173");
        assert!(page_matches_inspect_url(&listed, "http://localhost:5173"));
        assert_eq!(
            find_page_for_url(std::slice::from_ref(&listed), "http://localhost:5173")
                .map(|item| item.id.as_str()),
            Some("tab:1")
        );
    }
}
