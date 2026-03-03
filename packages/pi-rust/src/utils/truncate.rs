// Truncation utilities
/// Truncate a string to at most `max_bytes` bytes, appending a note if truncated.
pub fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Find a valid UTF-8 boundary at or before max_bytes
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[Truncated: output exceeds {} bytes]", &s[..end], max_bytes)
}

/// Truncate lines to at most `max_lines`, returning a note if truncated.
pub fn truncate_lines(s: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max_lines {
        return s.to_string();
    }
    format!(
        "{}\n[Truncated: showing {} of {} lines]",
        lines[..max_lines].join("\n"),
        max_lines,
        lines.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_bytes_within_limit() {
        let s = "hello";
        assert_eq!(truncate_bytes(s, 100), "hello");
    }

    #[test]
    fn test_truncate_bytes_exceeds_limit() {
        let s = "hello world";
        let result = truncate_bytes(s, 5);
        assert!(result.starts_with("hello"));
        assert!(result.contains("Truncated"));
    }

    #[test]
    fn test_truncate_lines_within_limit() {
        let s = "a\nb\nc";
        assert_eq!(truncate_lines(s, 10), "a\nb\nc");
    }

    #[test]
    fn test_truncate_lines_exceeds_limit() {
        let s = "a\nb\nc\nd\ne";
        let result = truncate_lines(s, 3);
        assert!(result.contains("Truncated"));
        assert!(result.contains("a\nb\nc"));
    }
}

