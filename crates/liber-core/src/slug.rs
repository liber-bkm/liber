const WINDOWS_RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

pub fn slugify(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::new();
    let mut last_dash = true;
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let mut short = if trimmed.len() > 60 {
        trimmed[..60].trim_matches('-').to_string()
    } else {
        trimmed.to_string()
    };
    if WINDOWS_RESERVED.contains(&short.as_str()) {
        short.push_str("-item");
    }
    short
}

pub fn slug_or_fallback(title: &str) -> String {
    let s = slugify(title);
    if s.is_empty() {
        "bookmark".to_string()
    } else {
        s
    }
}

pub fn sanitize_folder(f: &str) -> String {
    let mut clean = Vec::new();
    for part in f.replace('\\', "/").split('/') {
        let p = part.trim().trim_matches('.').trim();
        if p.is_empty() {
            continue;
        }
        clean.push(sanitize_filename(p));
    }
    clean.join("/")
}

pub fn sanitize_filename(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if (c as u32) < 32 => '-',
            c => c,
        })
        .collect();
    s = s.trim().trim_matches('.').trim().to_string();
    if s.is_empty() {
        return "item".to_string();
    }
    if WINDOWS_RESERVED.contains(&s.to_lowercase().as_str()) {
        s.push_str("-item");
    }
    s
}

/// Trims every path segment of a hierarchical tag and collapses empty segments.
pub fn sanitize_tag(tag: &str) -> String {
    let mut clean = Vec::new();
    for part in tag.split('/') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        clean.push(p);
    }
    clean.join("/")
}

/// Sanitizes and case-insensitively dedupes a tag list, preserving first spelling.
pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    dedupe_strings(tags.into_iter().map(|t| sanitize_tag(&t)).collect())
}

/// Returns the suffix below `target` when `value` is `target` or a descendant.
///
/// An empty suffix means an exact match; descendants start with `/`.
pub fn path_rest<'a>(value: &'a str, target: &str, ignore_ascii_case: bool) -> Option<&'a str> {
    if value.len() < target.len() || !value.is_char_boundary(target.len()) {
        return None;
    }
    let (prefix, rest) = value.split_at(target.len());
    let matches = if ignore_ascii_case {
        prefix.eq_ignore_ascii_case(target)
    } else {
        prefix == target
    };
    if matches && (rest.is_empty() || rest.starts_with('/')) {
        Some(rest)
    } else {
        None
    }
}

/// Case-sensitive path membership used by folders.
pub fn path_in_subtree(value: &str, target: &str) -> bool {
    path_rest(value, target, false).is_some()
}

/// Case-insensitive path membership used by tags.
pub fn tag_in_subtree(value: &str, target: &str) -> bool {
    path_rest(value, target, true).is_some()
}

pub fn normalize_url(u: &str) -> String {
    let u = u.trim();
    if u.contains("://") {
        u.to_string()
    } else {
        format!("https://{u}")
    }
}

pub fn dedupe_strings(items: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for it in items {
        let trimmed = it.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_lowercase()) {
            out.push(trimmed);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_vectors() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
        assert_eq!(slugify("con"), "con-item");
        assert_eq!(slugify(""), "");
        assert_eq!(slug_or_fallback(""), "bookmark");
        assert_eq!(slug_or_fallback("My Title"), "my-title");
    }

    #[test]
    fn folder_vectors() {
        assert_eq!(sanitize_folder("tech/rust"), "tech/rust");
        assert_eq!(sanitize_folder("a\\b"), "a/b");
        assert_eq!(sanitize_folder("../x/./y"), "x/y");
        assert_eq!(sanitize_folder(""), "");
    }

    #[test]
    fn filename_windows_names() {
        assert_eq!(sanitize_filename("AUX"), "AUX-item");
        assert_eq!(sanitize_filename("a<b"), "a-b");
        assert_eq!(sanitize_filename("..."), "item");
    }

    #[test]
    fn url_scheme_default() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
        assert_eq!(normalize_url("http://example.com"), "http://example.com");
    }

    #[test]
    fn dedupe_case_insensitive() {
        assert_eq!(
            dedupe_strings(vec!["A".into(), "a".into(), " b ".into()]),
            vec!["A".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn tag_paths_normalize() {
        assert_eq!(sanitize_tag("dev/rust"), "dev/rust");
        assert_eq!(sanitize_tag(" dev // rust / "), "dev/rust");
        assert_eq!(sanitize_tag("/"), "");
        assert_eq!(sanitize_tag("#rust"), "#rust");
        assert_eq!(
            normalize_tags(vec![
                "Dev".into(),
                "dev".into(),
                "dev//rust".into(),
                " ".into(),
            ]),
            vec!["Dev".to_string(), "dev/rust".to_string()]
        );
    }

    #[test]
    fn path_subtree_membership() {
        assert!(path_in_subtree("tech", "tech"));
        assert!(path_in_subtree("tech/rust", "tech"));
        assert!(path_in_subtree("tech/rust/async", "tech"));
        assert!(!path_in_subtree("technology", "tech"));
        assert!(!path_in_subtree("tech", "tech/rust"));
        assert!(!path_in_subtree("Tech", "tech"));
        assert!(tag_in_subtree("Dev/Rust", "dev"));
        assert!(tag_in_subtree("dev/rust", "DEV"));
        assert!(!tag_in_subtree("developer", "dev"));
        assert_eq!(path_rest("tech/rust", "tech", false), Some("/rust"));
        assert_eq!(path_rest("tech", "tech", false), Some(""));
    }
}
