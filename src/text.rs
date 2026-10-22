/// Truncate on a char boundary and say so.
pub fn truncate(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => format!("{}\n[truncated: output exceeded {max_chars} chars]", &s[..idx]),
        None => s.to_string(),
    }
}
