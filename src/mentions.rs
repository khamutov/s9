//! @mention parser for comment bodies.
//!
//! Extracts `@login` references from text per DD 0.6 §8.1.
//! Login format matches the user login rules: alphanumeric, hyphens, underscores.

use regex::Regex;
use std::collections::HashSet;

/// Regex for `@login` mentions. Matches `@` followed by one or more word characters
/// (letters, digits, underscore) or hyphens. Must be at the start of text or preceded
/// by whitespace/punctuation (not part of an email address).
///
/// Uses a non-capturing group with alternation instead of lookbehind (unsupported by
/// the `regex` crate). Capture group 1 or 2 holds the login depending on context.
static MENTION_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    // Match @login at start of string or after whitespace/punctuation.
    // Group 1: at start of string. Group 2: after non-word char.
    Regex::new(r"(?:^|[\s(,;!?\[])@([a-zA-Z0-9_][a-zA-Z0-9_-]*)").unwrap()
});

/// Extracts unique `@login` mentions from text.
///
/// Returns deduplicated login strings (lowercased) in no particular order.
/// Ignores email-like patterns (e.g., `user@example.com` won't match).
pub fn parse_mentions(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for cap in MENTION_RE.captures_iter(text) {
        let login = cap[1].to_lowercase();
        if seen.insert(login.clone()) {
            result.push(login);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_mention() {
        let mentions = parse_mentions("Hey @alice, check this out");
        assert_eq!(mentions, vec!["alice"]);
    }

    #[test]
    fn multiple_mentions() {
        let mentions = parse_mentions("@alice @bob please review");
        assert_eq!(mentions.len(), 2);
        assert!(mentions.contains(&"alice".to_string()));
        assert!(mentions.contains(&"bob".to_string()));
    }

    #[test]
    fn deduplicates() {
        let mentions = parse_mentions("@alice and @alice again");
        assert_eq!(mentions, vec!["alice"]);
    }

    #[test]
    fn case_insensitive() {
        let mentions = parse_mentions("@Alice and @ALICE");
        assert_eq!(mentions, vec!["alice"]);
    }

    #[test]
    fn mention_at_start() {
        let mentions = parse_mentions("@admin fix this");
        assert_eq!(mentions, vec!["admin"]);
    }

    #[test]
    fn mention_with_underscore_and_hyphen() {
        let mentions = parse_mentions("cc @user_name @some-dev");
        assert_eq!(mentions.len(), 2);
        assert!(mentions.contains(&"user_name".to_string()));
        assert!(mentions.contains(&"some-dev".to_string()));
    }

    #[test]
    fn no_email_match() {
        // Email-like patterns should not produce mentions.
        let mentions = parse_mentions("Send to user@example.com");
        assert!(mentions.is_empty());
    }

    #[test]
    fn empty_text() {
        let mentions = parse_mentions("");
        assert!(mentions.is_empty());
    }

    #[test]
    fn no_mentions() {
        let mentions = parse_mentions("Just a regular comment with no mentions");
        assert!(mentions.is_empty());
    }

    #[test]
    fn mention_after_newline() {
        let mentions = parse_mentions("First line\n@bob second line");
        assert_eq!(mentions, vec!["bob"]);
    }

    #[test]
    fn mention_in_parentheses() {
        let mentions = parse_mentions("(cc @alice)");
        assert_eq!(mentions, vec!["alice"]);
    }
}
