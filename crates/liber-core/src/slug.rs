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
