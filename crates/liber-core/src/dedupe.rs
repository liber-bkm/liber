use url::Url;

use crate::model::Bookmark;
use crate::CoreError;

const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "mc_cid",
    "mc_eid",
    "igshid",
    "ref",
];

pub fn normalize_for_dedupe(raw: &str) -> String {
    let raw = raw.trim();
    let u = match Url::parse(raw) {
        Ok(u) => u,
        Err(_) => return raw.to_string(),
    };
    if u.host_str().is_none() {
        return raw.to_string();
    }
    let mut out = String::new();
    out.push_str(&u.scheme().to_lowercase());
    out.push_str("://");
    out.push_str(&u.host_str().unwrap_or_default().to_lowercase());
    if let Some(port) = u.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }
    let path = u.path().trim_end_matches('/');
    out.push_str(path);
    let mut pairs: Vec<(String, String)> = u
        .query_pairs()
        .into_owned()
        .filter(|(k, _)| !TRACKING_PARAMS.contains(&k.as_str()))
        .collect();
    if !pairs.is_empty() {
        pairs.sort();
        let mut enc = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &pairs {
            enc.append_pair(k, v);
        }
        out.push('?');
        out.push_str(&enc.finish());
    }
    out
}

pub fn find_duplicate<'a>(
    bookmarks: &'a [Bookmark],
    url: &str,
) -> Result<Option<&'a Bookmark>, CoreError> {
    let target = normalize_for_dedupe(url);
    Ok(bookmarks
        .iter()
        .find(|b| normalize_for_dedupe(&b.url) == target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_vectors() {
        let cases = [
            ("https://example.com/", "https://example.com"),
            ("https://example.com", "https://example.com"),
            ("HTTPS://EXAMPLE.COM/x", "https://example.com/x"),
            ("http://example.com:80/x", "http://example.com/x"),
            ("https://example.com:443/x", "https://example.com/x"),
            ("https://example.com:8443/x", "https://example.com:8443/x"),
            (
                "https://example.com/x?utm_source=a&y=1",
                "https://example.com/x?y=1",
            ),
            (
                "https://example.com/x?fbclid=1&y=1",
                "https://example.com/x?y=1",
            ),
            ("https://example.com/x#frag", "https://example.com/x"),
            ("https://example.com/X", "https://example.com/X"),
        ];
        for (input, want) in cases {
            assert_eq!(normalize_for_dedupe(input), want, "input {input:?}");
        }
    }

    #[test]
    fn unparseable_returns_trimmed() {
        assert_eq!(normalize_for_dedupe("  not a url  "), "not a url");
    }
}
