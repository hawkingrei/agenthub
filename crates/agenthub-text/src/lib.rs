pub fn truncate_chars(raw: &str, max_chars: usize) -> String {
    if raw.is_empty() || max_chars == 0 {
        return String::new();
    }
    raw.chars().take(max_chars).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::truncate_chars;

    #[test]
    fn truncate_chars_returns_empty_for_empty_or_zero_limit() {
        assert_eq!(truncate_chars("", 10), "");
        assert_eq!(truncate_chars("hello", 0), "");
    }

    #[test]
    fn truncate_chars_preserves_short_input_and_cuts_long_input() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("hello", 3), "hel");
    }

    #[test]
    fn truncate_chars_respects_char_boundaries() {
        assert_eq!(truncate_chars("你好世界", 3), "你好世");
    }
}
