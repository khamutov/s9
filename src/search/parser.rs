//! Search filter parser: micro-syntax → structured query.
//!
//! Parses a raw query string like `owner:alex status:new crash on startup`
//! into a typed [`ParsedQuery`] struct. The parser always succeeds — invalid
//! or unrecognized tokens gracefully degrade to free-text terms.

use crate::models::parse_estimation;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Result of parsing a search query string.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedQuery {
    /// Filter clauses (implicit AND between them).
    pub clauses: Vec<Clause>,
    /// Free-text search terms.
    pub text_terms: Vec<TextTerm>,
}

impl ParsedQuery {
    /// Returns `true` when the query contains free-text terms, which
    /// determines cursor vs offset pagination downstream.
    pub fn has_text_search(&self) -> bool {
        !self.text_terms.is_empty()
    }
}

/// A filter clause — either a single filter or an OR-group.
#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    Single(Filter),
    Or(Vec<Filter>),
}

/// A single filter with optional negation.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub negated: bool,
    pub condition: FilterCondition,
}

/// Typed filter conditions produced by the parser.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterCondition {
    User {
        field: UserField,
        login: String,
    },
    Status(String),
    Priority(String),
    TicketType(String),
    Component(String),
    Milestone(String),
    Slug(Vec<String>),
    Is(IsCondition),
    DateComparison {
        field: DateField,
        op: ComparisonOp,
        value: String,
    },
    EstimationComparison {
        op: ComparisonOp,
        hours: f64,
    },
    Has(HasField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserField {
    Owner,
    Cc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsCondition {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateField {
    Created,
    Updated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Gt,
    Lt,
    Gte,
    Lte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasField {
    Estimation,
    Milestone,
}

/// A free-text search term.
#[derive(Debug, Clone, PartialEq)]
pub struct TextTerm {
    pub negated: bool,
    /// Content without surrounding quotes.
    pub text: String,
    /// `true` when the original token was enclosed in double quotes.
    pub is_phrase: bool,
}

// ---------------------------------------------------------------------------
// Recognized filter keys
// ---------------------------------------------------------------------------

const KNOWN_KEYS: &[&str] = &[
    "owner",
    "cc",
    "status",
    "priority",
    "type",
    "component",
    "milestone",
    "slug",
    "is",
    "created",
    "updated",
    "estimation",
    "has",
];

fn is_known_key(key: &str) -> bool {
    KNOWN_KEYS.contains(&key)
}

// ---------------------------------------------------------------------------
// Tokenizer (Pass 1)
// ---------------------------------------------------------------------------

/// Splits input on whitespace, respecting double-quoted strings.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

// ---------------------------------------------------------------------------
// Parser helpers
// ---------------------------------------------------------------------------

/// Strips surrounding double-quotes from a filter value.
fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}

/// Extracts a comparison operator prefix from a value string.
/// Returns `(op, remaining_value)`.
fn parse_comparison_prefix(value: &str) -> Option<(ComparisonOp, &str)> {
    if let Some(rest) = value.strip_prefix(">=") {
        Some((ComparisonOp::Gte, rest))
    } else if let Some(rest) = value.strip_prefix("<=") {
        Some((ComparisonOp::Lte, rest))
    } else if let Some(rest) = value.strip_prefix('>') {
        Some((ComparisonOp::Gt, rest))
    } else if let Some(rest) = value.strip_prefix('<') {
        Some((ComparisonOp::Lt, rest))
    } else {
        None
    }
}

/// Attempts to parse a `key:value` token into a [`Filter`].
/// Returns `None` if the key is unrecognized or the value is invalid,
/// in which case the caller should treat the token as free-text.
fn try_parse_filter(key: &str, raw_value: &str, negated: bool) -> Option<Filter> {
    let value = strip_quotes(raw_value);

    let condition = match key {
        "owner" => FilterCondition::User {
            field: UserField::Owner,
            login: value.to_string(),
        },
        "cc" => FilterCondition::User {
            field: UserField::Cc,
            login: value.to_string(),
        },
        "status" => FilterCondition::Status(value.to_string()),
        "priority" => FilterCondition::Priority(value.to_string()),
        "type" => FilterCondition::TicketType(value.to_string()),
        "component" => FilterCondition::Component(value.to_string()),
        "milestone" => FilterCondition::Milestone(value.to_string()),
        "slug" => {
            let slugs: Vec<String> = value.split(',').map(|s| s.to_string()).collect();
            FilterCondition::Slug(slugs)
        }
        "is" => match value {
            "open" => FilterCondition::Is(IsCondition::Open),
            "closed" => FilterCondition::Is(IsCondition::Closed),
            _ => return None,
        },
        "has" => match value {
            "estimation" => FilterCondition::Has(HasField::Estimation),
            "milestone" => FilterCondition::Has(HasField::Milestone),
            _ => return None,
        },
        "created" | "updated" => {
            let (op, date_value) = parse_comparison_prefix(value)?;
            let field = if key == "created" {
                DateField::Created
            } else {
                DateField::Updated
            };
            FilterCondition::DateComparison {
                field,
                op,
                value: date_value.to_string(),
            }
        }
        "estimation" => {
            let (op, est_value) = parse_comparison_prefix(value)?;
            let hours = parse_estimation(est_value).ok()?;
            FilterCondition::EstimationComparison { op, hours }
        }
        _ => return None,
    };

    Some(Filter { negated, condition })
}

// ---------------------------------------------------------------------------
// Parser (Pass 2)
// ---------------------------------------------------------------------------

/// Represents an intermediate parsed item before OR-grouping.
enum ParsedItem {
    Filter(Filter),
    TextTerm(TextTerm),
    OrMarker,
}

/// Parses a search query string into a structured [`ParsedQuery`].
///
/// The parser always succeeds. Unrecognized keys, invalid values, and
/// malformed tokens degrade gracefully to free-text terms.
pub fn parse(input: &str) -> ParsedQuery {
    let tokens = tokenize(input);
    let mut items: Vec<ParsedItem> = Vec::new();

    for token in &tokens {
        // OR marker — only when it is exactly "OR" (uppercase).
        if token == "OR" {
            items.push(ParsedItem::OrMarker);
            continue;
        }

        // Check for negation prefix.
        let (negated, body) = if let Some(rest) = token.strip_prefix('-') {
            if rest.is_empty() {
                // Bare "-" is free-text.
                items.push(ParsedItem::TextTerm(TextTerm {
                    negated: false,
                    text: "-".to_string(),
                    is_phrase: false,
                }));
                continue;
            }
            (true, rest)
        } else {
            (false, token.as_str())
        };

        // Try key:value split (first colon only).
        if let Some(colon_pos) = body.find(':') {
            let key = &body[..colon_pos];
            let raw_value = &body[colon_pos + 1..];

            if is_known_key(key)
                && !raw_value.is_empty()
                && let Some(filter) = try_parse_filter(key, raw_value, negated)
            {
                items.push(ParsedItem::Filter(filter));
                continue;
            }
        }

        // Quoted phrase or plain free-text.
        let is_phrase = body.starts_with('"') && body.ends_with('"') && body.len() >= 2;
        let text = if is_phrase {
            body[1..body.len() - 1].to_string()
        } else {
            body.to_string()
        };

        items.push(ParsedItem::TextTerm(TextTerm {
            negated,
            text,
            is_phrase,
        }));
    }

    // Group OR chains: Filter OR Filter OR Filter → Clause::Or(vec![...])
    build_query(items)
}

/// Walks the parsed items and groups adjacent filters connected by OR markers.
fn build_query(items: Vec<ParsedItem>) -> ParsedQuery {
    let mut clauses: Vec<Clause> = Vec::new();
    let mut text_terms: Vec<TextTerm> = Vec::new();

    let len = items.len();
    let mut i = 0;

    while i < len {
        match &items[i] {
            ParsedItem::Filter(_) => {
                // Collect an OR-chain of filters starting at this position.
                let mut chain: Vec<Filter> = Vec::new();

                // Take the first filter.
                if let ParsedItem::Filter(f) = &items[i] {
                    chain.push(f.clone());
                }
                i += 1;

                // Consume consecutive `OR Filter` pairs.
                while i + 1 < len {
                    if matches!(&items[i], ParsedItem::OrMarker) {
                        if let ParsedItem::Filter(f) = &items[i + 1] {
                            chain.push(f.clone());
                            i += 2;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if chain.len() == 1 {
                    clauses.push(Clause::Single(chain.remove(0)));
                } else {
                    clauses.push(Clause::Or(chain));
                }
            }
            ParsedItem::TextTerm(t) => {
                text_terms.push(t.clone());
                i += 1;
            }
            ParsedItem::OrMarker => {
                // OR at start, end, or not between two filters → free-text.
                text_terms.push(TextTerm {
                    negated: false,
                    text: "OR".to_string(),
                    is_phrase: false,
                });
                i += 1;
            }
        }
    }

    ParsedQuery {
        clauses,
        text_terms,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Tokenizer --------------------------------------------------------

    #[test]
    fn tokenize_empty() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   ").is_empty());
    }

    #[test]
    fn tokenize_single_word() {
        assert_eq!(tokenize("hello"), vec!["hello"]);
    }

    #[test]
    fn tokenize_multiple_words() {
        assert_eq!(tokenize("crash on startup"), vec!["crash", "on", "startup"]);
    }

    #[test]
    fn tokenize_quoted_phrase() {
        assert_eq!(tokenize(r#""stack overflow""#), vec!["\"stack overflow\""]);
    }

    #[test]
    fn tokenize_mixed() {
        assert_eq!(
            tokenize(r#"status:new "hello world" foo"#),
            vec!["status:new", "\"hello world\"", "foo"]
        );
    }

    #[test]
    fn tokenize_unclosed_quote() {
        // Unclosed quote: everything after the opening quote stays in one token.
        assert_eq!(tokenize(r#""hello world"#), vec!["\"hello world"]);
    }

    // ---- Simple filters ---------------------------------------------------

    #[test]
    fn filter_owner() {
        let q = parse("owner:alex");
        assert_eq!(q.clauses.len(), 1);
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                negated: false,
                condition: FilterCondition::User { field: UserField::Owner, login }
            }) if login == "alex"
        ));
        assert!(q.text_terms.is_empty());
    }

    #[test]
    fn filter_cc() {
        let q = parse("cc:maria");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::User { field: UserField::Cc, login }, ..
            }) if login == "maria"
        ));
    }

    #[test]
    fn filter_status() {
        let q = parse("status:in_progress");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { condition: FilterCondition::Status(s), .. }) if s == "in_progress"
        ));
    }

    #[test]
    fn filter_priority() {
        let q = parse("priority:P0");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { condition: FilterCondition::Priority(s), .. }) if s == "P0"
        ));
    }

    #[test]
    fn filter_ticket_type() {
        let q = parse("type:bug");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { condition: FilterCondition::TicketType(s), .. }) if s == "bug"
        ));
    }

    #[test]
    fn filter_component() {
        let q = parse("component:Platform/DNS");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { condition: FilterCondition::Component(s), .. }) if s == "Platform/DNS"
        ));
    }

    #[test]
    fn filter_milestone() {
        let q = parse("milestone:v2.0");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { condition: FilterCondition::Milestone(s), .. }) if s == "v2.0"
        ));
    }

    #[test]
    fn filter_slug_single() {
        let q = parse("slug:MAP");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { condition: FilterCondition::Slug(slugs), .. })
                if slugs == &["MAP"]
        ));
    }

    #[test]
    fn filter_slug_multi() {
        let q = parse("slug:MAP,NET,API");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { condition: FilterCondition::Slug(slugs), .. })
                if slugs == &["MAP", "NET", "API"]
        ));
    }

    #[test]
    fn filter_is_open() {
        let q = parse("is:open");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::Is(IsCondition::Open),
                ..
            })
        ));
    }

    #[test]
    fn filter_is_closed() {
        let q = parse("is:closed");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::Is(IsCondition::Closed),
                ..
            })
        ));
    }

    #[test]
    fn filter_has_estimation() {
        let q = parse("has:estimation");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::Has(HasField::Estimation),
                ..
            })
        ));
    }

    #[test]
    fn filter_has_milestone() {
        let q = parse("has:milestone");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::Has(HasField::Milestone),
                ..
            })
        ));
    }

    #[test]
    fn filter_quoted_value() {
        let q = parse(r#"owner:"alex""#);
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::User { field: UserField::Owner, login }, ..
            }) if login == "alex"
        ));
    }

    // ---- Comparison filters -----------------------------------------------

    #[test]
    fn date_created_gt() {
        let q = parse("created:>2026-01-01");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::DateComparison {
                    field: DateField::Created,
                    op: ComparisonOp::Gt,
                    value,
                }, ..
            }) if value == "2026-01-01"
        ));
    }

    #[test]
    fn date_updated_lte() {
        let q = parse("updated:<=2026-03-01");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::DateComparison {
                    field: DateField::Updated,
                    op: ComparisonOp::Lte,
                    value,
                }, ..
            }) if value == "2026-03-01"
        ));
    }

    #[test]
    fn date_created_lt() {
        let q = parse("created:<2026-06-15");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::DateComparison {
                    field: DateField::Created,
                    op: ComparisonOp::Lt,
                    value,
                }, ..
            }) if value == "2026-06-15"
        ));
    }

    #[test]
    fn date_updated_gte() {
        let q = parse("updated:>=2025-12-01");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::DateComparison {
                    field: DateField::Updated,
                    op: ComparisonOp::Gte,
                    value,
                }, ..
            }) if value == "2025-12-01"
        ));
    }

    #[test]
    fn estimation_gt_duration() {
        let q = parse("estimation:>2d");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::EstimationComparison {
                    op: ComparisonOp::Gt,
                    hours,
                }, ..
            }) if (*hours - 16.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn estimation_lte_complex_duration() {
        let q = parse("estimation:<=1w2d");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::EstimationComparison {
                    op: ComparisonOp::Lte,
                    hours,
                }, ..
            }) if (*hours - 56.0).abs() < f64::EPSILON // 1w=40h + 2d=16h
        ));
    }

    // ---- Negation ---------------------------------------------------------

    #[test]
    fn negated_filter() {
        let q = parse("-status:done");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                negated: true,
                condition: FilterCondition::Status(s),
            }) if s == "done"
        ));
    }

    #[test]
    fn negated_text() {
        let q = parse("-crash");
        assert_eq!(q.text_terms.len(), 1);
        assert!(q.text_terms[0].negated);
        assert_eq!(q.text_terms[0].text, "crash");
    }

    #[test]
    fn negated_filter_in_or_group() {
        let q = parse("status:new OR -status:done");
        assert!(matches!(&q.clauses[0], Clause::Or(filters) if filters.len() == 2));
        if let Clause::Or(filters) = &q.clauses[0] {
            assert!(!filters[0].negated);
            assert!(filters[1].negated);
        }
    }

    // ---- OR grouping ------------------------------------------------------

    #[test]
    fn or_two_filters() {
        let q = parse("status:new OR status:in_progress");
        assert_eq!(q.clauses.len(), 1);
        assert!(matches!(&q.clauses[0], Clause::Or(filters) if filters.len() == 2));
    }

    #[test]
    fn or_three_chained() {
        let q = parse("priority:P0 OR priority:P1 OR priority:P2");
        assert_eq!(q.clauses.len(), 1);
        if let Clause::Or(filters) = &q.clauses[0] {
            assert_eq!(filters.len(), 3);
        } else {
            panic!("expected Or clause");
        }
    }

    #[test]
    fn or_mixed_keys() {
        let q = parse("status:new OR priority:P0");
        assert_eq!(q.clauses.len(), 1);
        assert!(matches!(&q.clauses[0], Clause::Or(_)));
    }

    #[test]
    fn or_adjacent_to_non_filter_degrades() {
        // "status:new OR crash" → Single(status:new), text "OR", text "crash"
        let q = parse("status:new OR crash");
        assert_eq!(q.clauses.len(), 1);
        assert!(matches!(&q.clauses[0], Clause::Single(_)));
        assert_eq!(q.text_terms.len(), 2);
        assert_eq!(q.text_terms[0].text, "OR");
        assert_eq!(q.text_terms[1].text, "crash");
    }

    // ---- Free-text --------------------------------------------------------

    #[test]
    fn free_text_bare_words() {
        let q = parse("crash on startup");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 3);
        assert_eq!(q.text_terms[0].text, "crash");
        assert!(!q.text_terms[0].is_phrase);
    }

    #[test]
    fn free_text_quoted_phrase() {
        let q = parse(r#""stack overflow""#);
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "stack overflow");
        assert!(q.text_terms[0].is_phrase);
    }

    #[test]
    fn free_text_mixed_with_filters() {
        let q = parse("owner:alex crash on startup");
        assert_eq!(q.clauses.len(), 1);
        assert_eq!(q.text_terms.len(), 3);
    }

    // ---- Combined ---------------------------------------------------------

    #[test]
    fn combined_query() {
        let q = parse("owner:alex status:new crash on startup");
        assert_eq!(q.clauses.len(), 2);
        assert_eq!(q.text_terms.len(), 3);
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::User {
                    field: UserField::Owner,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            &q.clauses[1],
            Clause::Single(Filter {
                condition: FilterCondition::Status(_),
                ..
            })
        ));
    }

    #[test]
    fn combined_negation_or_phrase() {
        let q = parse(r#"-status:done priority:P0 OR priority:P1 "null pointer""#);
        // -status:done → Single (negated)
        // priority:P0 OR priority:P1 → Or([P0, P1])
        // "null pointer" → phrase text term
        assert_eq!(q.clauses.len(), 2);
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter { negated: true, condition: FilterCondition::Status(s) })
                if s == "done"
        ));
        assert!(matches!(&q.clauses[1], Clause::Or(filters) if filters.len() == 2));
        assert_eq!(q.text_terms.len(), 1);
        assert!(q.text_terms[0].is_phrase);
        assert_eq!(q.text_terms[0].text, "null pointer");
    }

    // ---- has_text_search --------------------------------------------------

    #[test]
    fn has_text_search_filters_only() {
        let q = parse("status:new owner:alex");
        assert!(!q.has_text_search());
    }

    #[test]
    fn has_text_search_with_text() {
        let q = parse("status:new crash");
        assert!(q.has_text_search());
    }

    #[test]
    fn has_text_search_empty() {
        let q = parse("");
        assert!(!q.has_text_search());
    }

    // ---- Edge cases -------------------------------------------------------

    #[test]
    fn unknown_key_becomes_text() {
        let q = parse("foo:bar");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "foo:bar");
    }

    #[test]
    fn bad_estimation_becomes_text() {
        let q = parse("estimation:>abc");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "estimation:>abc");
    }

    #[test]
    fn or_at_start() {
        let q = parse("OR status:new");
        assert_eq!(q.clauses.len(), 1);
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "OR");
    }

    #[test]
    fn or_at_end() {
        let q = parse("status:new OR");
        assert_eq!(q.clauses.len(), 1);
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "OR");
    }

    #[test]
    fn empty_input() {
        let q = parse("");
        assert!(q.clauses.is_empty());
        assert!(q.text_terms.is_empty());
    }

    #[test]
    fn colon_in_value() {
        // component:Platform/DNS:8080 → split on first colon only
        let q = parse("component:Platform/DNS:8080");
        assert!(matches!(
            &q.clauses[0],
            Clause::Single(Filter {
                condition: FilterCondition::Component(s), ..
            }) if s == "Platform/DNS:8080"
        ));
    }

    #[test]
    fn time_like_token_becomes_text() {
        let q = parse("10:30");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "10:30");
    }

    #[test]
    fn is_unknown_value_becomes_text() {
        let q = parse("is:whatever");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "is:whatever");
    }

    #[test]
    fn has_unknown_value_becomes_text() {
        let q = parse("has:something");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "has:something");
    }

    #[test]
    fn estimation_without_operator_becomes_text() {
        let q = parse("estimation:2d");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "estimation:2d");
    }

    #[test]
    fn date_without_operator_becomes_text() {
        let q = parse("created:2026-01-01");
        assert!(q.clauses.is_empty());
        assert_eq!(q.text_terms.len(), 1);
        assert_eq!(q.text_terms[0].text, "created:2026-01-01");
    }
}
