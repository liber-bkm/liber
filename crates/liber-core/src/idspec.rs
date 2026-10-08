use crate::CoreError;

pub const MAX_RANGE_SPAN: i64 = 1000;

fn expand_range_token(token: &str) -> Result<Option<Vec<String>>, CoreError> {
    let Some((left, right)) = token.split_once('-') else {
        return Ok(None);
    };
    let (left, right) = (left.trim(), right.trim());
    if left.is_empty()
        || right.is_empty()
        || !left.bytes().all(|b| b.is_ascii_digit())
        || !right.bytes().all(|b| b.is_ascii_digit())
    {
        return Ok(None);
    }
    let start: i64 = left
        .parse()
        .map_err(|_| CoreError::Invalid(format!("bad range {token:?}")))?;
    let end: i64 = right
        .parse()
        .map_err(|_| CoreError::Invalid(format!("bad range {token:?}")))?;
    if end < start {
        return Err(CoreError::Invalid(format!("reversed range {token:?}")));
    }
    if end - start >= MAX_RANGE_SPAN {
        return Err(CoreError::Invalid(format!(
            "range {token:?} spans more than {MAX_RANGE_SPAN} ids"
        )));
    }
    Ok(Some((start..=end).map(|n| n.to_string()).collect()))
}

pub fn parse_id_spec(spec: &str) -> Result<Vec<String>, CoreError> {
    let mut out: Vec<String> = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match expand_range_token(part)? {
            Some(ids) => out.extend(ids),
            None => out.push(part.to_string()),
        }
    }
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
    fn parse_ranges() {
        assert_eq!(parse_id_spec("1-3").unwrap(), vec!["1", "2", "3"]);
        assert_eq!(
            parse_id_spec("1-3,2").unwrap(),
            vec!["1".to_string(), "2".to_string(), "3".to_string()]
        );
        assert_eq!(parse_id_spec("2 - 4").unwrap(), vec!["2", "3", "4"]);
        assert_eq!(parse_id_spec("5").unwrap(), vec!["5"]);
        assert!(parse_id_spec("3-1").is_err());
        assert!(parse_id_spec("1-1001").is_err());
        assert_eq!(parse_id_spec("1-1000").unwrap().len(), 1000);
    }

    #[test]
    fn non_numeric_dashes_stay_tokens() {
        assert_eq!(parse_id_spec("a1-b2").unwrap(), vec!["a1-b2"]);
        assert_eq!(parse_id_spec("12-ab").unwrap(), vec!["12-ab"]);
        assert_eq!(parse_id_spec("1-2-3").unwrap(), vec!["1-2-3"]);
        assert_eq!(parse_id_spec("-3").unwrap(), vec!["-3"]);
        assert_eq!(parse_id_spec("3-").unwrap(), vec!["3-"]);
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
