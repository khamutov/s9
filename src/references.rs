#![allow(dead_code)]

//! Micro-syntax reference parser for ticket/comment cross-references.
//!
//! Extracts `#ID`, `#PREFIX-ID`, `comment#N`, and `#ID/comment#N` references
//! from text per PRD §4.1 and DD 0.17 §7.5.

use regex::Regex;
use std::collections::HashSet;

/// A parsed reference found in text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Reference {
    /// `#42` — numeric ticket reference.
    Ticket { id: i64 },
    /// `#MAP-42` — slug-prefixed ticket reference.
    SlugTicket { prefix: String, id: i64 },
    /// `comment#3` — comment in the current ticket context.
    Comment { number: i64 },
    /// `#42/comment#3` or `#MAP-42/comment#3` — comment in a specific ticket.
    TicketComment {
        prefix: Option<String>,
        ticket_id: i64,
        comment_number: i64,
    },
}

/// Regex that matches all reference patterns. Ordering ensures longer/more-specific
/// patterns match first.
///
/// Patterns (in priority order):
/// 1. `#PREFIX-ID/comment#N` — slug ticket + comment
/// 2. `#ID/comment#N` — numeric ticket + comment
/// 3. `#PREFIX-ID` — slug ticket
/// 4. `#ID` — numeric ticket
/// 5. `comment#N` — comment only
///
/// All patterns require a word boundary or start-of-text before them to avoid
/// matching inside URLs or other tokens.
static REFERENCE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    // Uses a non-capturing prefix group (like mentions.rs) instead of lookbehind.
    // The longer patterns (ticket+comment) are listed first so they match before
    // shorter patterns (#ID alone). The `regex` crate uses leftmost-first alternation.
    Regex::new(concat!(
        r"(?:^|[\s(,;!?\[])",
        r"(?:",
        // Group 1,2,3: #PREFIX-ID/comment#N
        r"#([A-Z][A-Z0-9]+)-(\d+)/comment#(\d+)",
        r"|",
        // Group 4,5: #ID/comment#N
        r"#(\d+)/comment#(\d+)",
        r"|",
        // Group 6,7: #PREFIX-ID
        r"#([A-Z][A-Z0-9]+)-(\d+)",
        r"|",
        // Group 8: #ID
        r"#(\d+)",
        r"|",
        // Group 9: comment#N (standalone)
        r"comment#(\d+)",
        r")",
    ))
    .unwrap()
});

/// Extracts unique references from text.
///
/// Returns deduplicated references in the order they first appear.
/// Invalid IDs (zero, overflow) are silently skipped.
pub fn parse_references(text: &str) -> Vec<Reference> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for cap in REFERENCE_RE.captures_iter(text) {
        let reference =
            if let (Some(prefix), Some(id), Some(comment)) = (cap.get(1), cap.get(2), cap.get(3)) {
                // #PREFIX-ID/comment#N
                let Ok(ticket_id) = id.as_str().parse::<i64>() else {
                    continue;
                };
                let Ok(comment_number) = comment.as_str().parse::<i64>() else {
                    continue;
                };
                if ticket_id == 0 || comment_number == 0 {
                    continue;
                }
                Reference::TicketComment {
                    prefix: Some(prefix.as_str().to_string()),
                    ticket_id,
                    comment_number,
                }
            } else if let (Some(id), Some(comment)) = (cap.get(4), cap.get(5)) {
                // #ID/comment#N
                let Ok(ticket_id) = id.as_str().parse::<i64>() else {
                    continue;
                };
                let Ok(comment_number) = comment.as_str().parse::<i64>() else {
                    continue;
                };
                if ticket_id == 0 || comment_number == 0 {
                    continue;
                }
                Reference::TicketComment {
                    prefix: None,
                    ticket_id,
                    comment_number,
                }
            } else if let (Some(prefix), Some(id)) = (cap.get(6), cap.get(7)) {
                // #PREFIX-ID
                let Ok(id) = id.as_str().parse::<i64>() else {
                    continue;
                };
                if id == 0 {
                    continue;
                }
                Reference::SlugTicket {
                    prefix: prefix.as_str().to_string(),
                    id,
                }
            } else if let Some(id) = cap.get(8) {
                // #ID
                let Ok(id) = id.as_str().parse::<i64>() else {
                    continue;
                };
                if id == 0 {
                    continue;
                }
                Reference::Ticket { id }
            } else if let Some(num) = cap.get(9) {
                // comment#N
                let Ok(number) = num.as_str().parse::<i64>() else {
                    continue;
                };
                if number == 0 {
                    continue;
                }
                Reference::Comment { number }
            } else {
                continue;
            };

        if seen.insert(reference.clone()) {
            result.push(reference);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_ticket_reference() {
        let refs = parse_references("See #42 for details");
        assert_eq!(refs, vec![Reference::Ticket { id: 42 }]);
    }

    #[test]
    fn slug_ticket_reference() {
        let refs = parse_references("Fixed in #MAP-23");
        assert_eq!(
            refs,
            vec![Reference::SlugTicket {
                prefix: "MAP".to_string(),
                id: 23,
            }]
        );
    }

    #[test]
    fn comment_reference() {
        let refs = parse_references("See comment#5 above");
        assert_eq!(refs, vec![Reference::Comment { number: 5 }]);
    }

    #[test]
    fn ticket_comment_reference() {
        let refs = parse_references("As noted in #42/comment#3");
        assert_eq!(
            refs,
            vec![Reference::TicketComment {
                prefix: None,
                ticket_id: 42,
                comment_number: 3,
            }]
        );
    }

    #[test]
    fn slug_ticket_comment_reference() {
        let refs = parse_references("See #MAP-23/comment#1");
        assert_eq!(
            refs,
            vec![Reference::TicketComment {
                prefix: Some("MAP".to_string()),
                ticket_id: 23,
                comment_number: 1,
            }]
        );
    }

    #[test]
    fn multiple_references() {
        let refs = parse_references("Related to #10 and #MAP-23, also comment#2");
        assert_eq!(refs.len(), 3);
        assert!(refs.contains(&Reference::Ticket { id: 10 }));
        assert!(refs.contains(&Reference::SlugTicket {
            prefix: "MAP".to_string(),
            id: 23,
        }));
        assert!(refs.contains(&Reference::Comment { number: 2 }));
    }

    #[test]
    fn deduplicates() {
        let refs = parse_references("#42 and then #42 again");
        assert_eq!(refs, vec![Reference::Ticket { id: 42 }]);
    }

    #[test]
    fn at_start_of_text() {
        let refs = parse_references("#100 is the main issue");
        assert_eq!(refs, vec![Reference::Ticket { id: 100 }]);
    }

    #[test]
    fn after_newline() {
        let refs = parse_references("Line one\n#55 on line two");
        assert_eq!(refs, vec![Reference::Ticket { id: 55 }]);
    }

    #[test]
    fn in_parentheses() {
        let refs = parse_references("(see #7)");
        assert_eq!(refs, vec![Reference::Ticket { id: 7 }]);
    }

    #[test]
    fn multi_letter_prefix() {
        let refs = parse_references("Blocked by #PLAT-107");
        assert_eq!(
            refs,
            vec![Reference::SlugTicket {
                prefix: "PLAT".to_string(),
                id: 107,
            }]
        );
    }

    #[test]
    fn prefix_with_digits() {
        let refs = parse_references("See #V2-5");
        assert_eq!(
            refs,
            vec![Reference::SlugTicket {
                prefix: "V2".to_string(),
                id: 5,
            }]
        );
    }

    #[test]
    fn zero_id_skipped() {
        let refs = parse_references("#0 and comment#0");
        assert!(refs.is_empty());
    }

    #[test]
    fn no_references() {
        let refs = parse_references("Just a regular comment with no references");
        assert!(refs.is_empty());
    }

    #[test]
    fn empty_text() {
        let refs = parse_references("");
        assert!(refs.is_empty());
    }

    #[test]
    fn does_not_match_email_hash() {
        // A # inside a URL or unrelated context should not match.
        let refs = parse_references("Visit page#section for more");
        // "page#section" — the # is preceded by 'e', not whitespace, so no match.
        assert!(refs.is_empty());
    }

    #[test]
    fn slug_takes_priority_over_numeric() {
        // #MAP-23 should be parsed as SlugTicket, not Ticket { id: 23 }
        let refs = parse_references("#MAP-23");
        assert_eq!(
            refs,
            vec![Reference::SlugTicket {
                prefix: "MAP".to_string(),
                id: 23,
            }]
        );
    }

    #[test]
    fn ticket_comment_takes_priority() {
        // #42/comment#3 should be one TicketComment, not a Ticket + Comment
        let refs = parse_references("#42/comment#3");
        assert_eq!(
            refs,
            vec![Reference::TicketComment {
                prefix: None,
                ticket_id: 42,
                comment_number: 3,
            }]
        );
    }

    #[test]
    fn mixed_references_in_paragraph() {
        let text = "This relates to #MAP-10 and #5. See comment#2 for context, \
                    or #PLAT-99/comment#1 for the original discussion.";
        let refs = parse_references(text);
        assert_eq!(refs.len(), 4);
        assert!(refs.contains(&Reference::SlugTicket {
            prefix: "MAP".to_string(),
            id: 10,
        }));
        assert!(refs.contains(&Reference::Ticket { id: 5 }));
        assert!(refs.contains(&Reference::Comment { number: 2 }));
        assert!(refs.contains(&Reference::TicketComment {
            prefix: Some("PLAT".to_string()),
            ticket_id: 99,
            comment_number: 1,
        }));
    }

    #[test]
    fn comment_reference_at_start() {
        let refs = parse_references("comment#7 has the answer");
        assert_eq!(refs, vec![Reference::Comment { number: 7 }]);
    }
}
