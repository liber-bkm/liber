use crate::CoreError;

pub fn parse_id_spec(spec: &str) -> Result<Vec<String>, CoreError> {
    let mut out: Vec<String> = spec
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    if out.is_empty() {
        return Err(CoreError::Invalid("no ids given".to_string()));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

pub fn is_query_spec(spec: &str) -> bool {
    for c in spec.chars() {
        if !(c.is_ascii_hexdigit() || c == ',' || c == '-' || c == ' ') {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tokens() {
        assert_eq!(parse_id_spec("ab12").unwrap(), vec!["ab12"]);
        assert_eq!(
            parse_id_spec("b2,a1,b2").unwrap(),
            vec!["a1".to_string(), "b2".to_string()]
        );
        assert!(parse_id_spec("").is_err());
        assert!(parse_id_spec(" , ").is_err());
    }

    #[test]
    fn query_detection() {
        for s in ["ab12", "a1-b2", "b2,a1", "a1, b2"] {
            assert!(!is_query_spec(s), "{s:?} should not be a query");
        }
        for s in ["alpha", "my query", "1-4,x", "v2", "example.com"] {
            assert!(is_query_spec(s), "{s:?} should be a query");
        }
    }
}
