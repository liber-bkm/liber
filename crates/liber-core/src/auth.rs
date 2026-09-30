use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

pub const COOKIE_NAME: &str = "liber_auth";

pub fn auth_mac(token: &str, purpose: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("hmac takes any key");
    mac.update(purpose.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn check_bearer(header_value: &str, token: &str) -> bool {
    let hex_part = header_value
        .strip_prefix("Bearer")
        .unwrap_or(header_value)
        .trim();
    let Ok(got) = hex::decode(hex_part) else {
        return false;
    };
    if got.is_empty() {
        return false;
    }
    let want = hex::decode(auth_mac(token, "liber-bearer-v1")).unwrap_or_default();
    got.as_slice().ct_eq(want.as_slice()).into()
}

pub fn check_cookie(cookie_value: &str, token: &str) -> bool {
    let Ok(got) = hex::decode(cookie_value.trim()) else {
        return false;
    };
    let want = hex::decode(auth_mac(token, "liber-cookie-v1")).unwrap_or_default();
    got.as_slice().ct_eq(want.as_slice()).into()
}

pub fn check_token(candidate: &str, token: &str) -> bool {
    if candidate.is_empty() {
        return false;
    }
    candidate.as_bytes().ct_eq(token.as_bytes()).into()
}

pub fn same_origin(origin: Option<&str>, referer: Option<&str>, host: &str) -> bool {
    for raw in [origin, referer].into_iter().flatten() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let Ok(url) = url::Url::parse(raw) else {
            return false;
        };
        let Some(url_host) = url.host_str() else {
            return false;
        };
        let mut url_host = url_host.to_lowercase();
        if let Some(port) = url.port() {
            url_host.push(':');
            url_host.push_str(&port.to_string());
        }
        if !url_host.eq_ignore_ascii_case(host.trim()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_roundtrip() {
        let token = "secret-token";
        let bearer = format!("Bearer {}", auth_mac(token, "liber-bearer-v1"));
        assert!(check_bearer(&bearer, token));
        assert!(!check_bearer(&bearer, "wrong"));
        assert!(!check_bearer("Bearer zzz", token));
        assert!(!check_bearer("", token));
    }

    #[test]
    fn cookie_roundtrip() {
        let token = "secret-token";
        let cookie = auth_mac(token, "liber-cookie-v1");
        assert!(check_cookie(&cookie, token));
        assert!(!check_cookie(&cookie, "wrong"));
        let bearer = auth_mac(token, "liber-bearer-v1");
        assert!(!check_cookie(&bearer, token));
    }

    #[test]
    fn origin_checks() {
        assert!(same_origin(
            Some("http://127.0.0.1:8080"),
            None,
            "127.0.0.1:8080"
        ));
        assert!(!same_origin(
            Some("http://evil.com"),
            None,
            "127.0.0.1:8080"
        ));
        assert!(!same_origin(
            None,
            Some("http://evil.com/x"),
            "127.0.0.1:8080"
        ));
        assert!(same_origin(None, None, "127.0.0.1:8080"));
    }
}
