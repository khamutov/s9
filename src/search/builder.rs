//! SQL query builder: [`ParsedQuery`] → dynamic SQL with FTS5 MATCH.
//!
//! Translates structured search queries into parameterized SQL. Supports
//! dual pagination: offset-based for FTS queries (relevance ranking),
//! cursor-based for structured-only queries (efficient scanning).

use std::fmt;

use super::parser::{
    Clause, ComparisonOp, DateField, Filter, FilterCondition, HasField, IsCondition, ParsedQuery,
    TextTerm, UserField,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A bind value for the generated SQL.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Text(String),
    Int(i64),
    Float(f64),
}

/// Result of translating a [`ParsedQuery`] into SQL.
#[derive(Debug, Clone)]
pub struct BuiltQuery {
    /// The main SELECT query.
    pub sql: String,
    /// Bind values for the main query, in positional order.
    pub binds: Vec<SqlValue>,
    /// Whether the query uses FTS (determines pagination mode).
    pub has_fts: bool,
    /// Count query for offset pagination (only set when `has_fts` is true).
    pub count_sql: Option<String>,
    /// Bind values for the count query.
    pub count_binds: Vec<SqlValue>,
}

/// Errors that can occur during query building.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// A filter references a feature not yet available.
    Unsupported(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(msg) => write!(f, "unsupported filter: {msg}"),
        }
    }
}

impl std::error::Error for BuildError {}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Builds a search SQL query from a parsed query.
///
/// - `page_size`: maximum rows to return.
/// - `offset`: row offset for FTS queries (offset pagination).
/// - `cursor`: `(updated_at_rfc3339, id)` for cursor pagination on non-FTS queries.
pub fn build_search_query(
    query: &ParsedQuery,
    page_size: i64,
    offset: Option<i64>,
    cursor: Option<(&str, i64)>,
) -> Result<BuiltQuery, BuildError> {
    let has_fts = query.has_text_search();
    let mut binds: Vec<SqlValue> = Vec::new();
    let mut where_parts: Vec<String> = Vec::new();

    // FTS MATCH clause.
    let fts_match = if has_fts {
        let m = build_fts_match(&query.text_terms);
        binds.push(SqlValue::Text(m.clone()));
        Some(m)
    } else {
        None
    };

    // Structured filter clauses.
    for clause in &query.clauses {
        let fragment = build_clause(clause, &mut binds)?;
        where_parts.push(fragment);
    }

    // Assemble the full query.
    if has_fts {
        build_fts_query(
            &where_parts,
            &binds,
            fts_match.as_deref(),
            page_size,
            offset,
        )
    } else {
        Ok(build_cursor_query(&where_parts, binds, page_size, cursor))
    }
}

// ---------------------------------------------------------------------------
// FTS MATCH construction
// ---------------------------------------------------------------------------

/// Builds an FTS5 MATCH expression from free-text terms.
///
/// - Phrases → `"exact phrase"`
/// - Short terms (< 3 chars) → `term*` (prefix search)
/// - Negated terms → `NOT term`
/// - All joined with space (FTS5 implicit AND)
fn build_fts_match(terms: &[TextTerm]) -> String {
    let parts: Vec<String> = terms
        .iter()
        .map(|t| {
            let expr = if t.is_phrase {
                format!("\"{}\"", t.text)
            } else if t.text.len() < 3 {
                format!("{}*", t.text)
            } else {
                t.text.clone()
            };

            if t.negated {
                format!("NOT {expr}")
            } else {
                expr
            }
        })
        .collect();

    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Filter → SQL fragment
// ---------------------------------------------------------------------------

/// Translates a single [`Clause`] into a SQL WHERE fragment.
fn build_clause(clause: &Clause, binds: &mut Vec<SqlValue>) -> Result<String, BuildError> {
    match clause {
        Clause::Single(filter) => build_filter(filter, binds),
        Clause::Or(filters) => {
            let parts: Vec<String> = filters
                .iter()
                .map(|f| build_filter(f, binds))
                .collect::<Result<_, _>>()?;
            Ok(format!("({})", parts.join(" OR ")))
        }
    }
}

/// Translates a single [`Filter`] into a SQL WHERE fragment.
fn build_filter(filter: &Filter, binds: &mut Vec<SqlValue>) -> Result<String, BuildError> {
    let raw = build_condition(&filter.condition, binds)?;
    if filter.negated {
        Ok(format!("NOT ({raw})"))
    } else {
        Ok(raw)
    }
}

/// Translates a [`FilterCondition`] into a SQL fragment, pushing bind values.
fn build_condition(
    condition: &FilterCondition,
    binds: &mut Vec<SqlValue>,
) -> Result<String, BuildError> {
    let fragment = match condition {
        FilterCondition::User { field, login } => {
            binds.push(SqlValue::Text(login.clone()));
            let pos = binds.len();
            match field {
                UserField::Owner => {
                    format!("t.owner_id IN (SELECT id FROM users WHERE login = ?{pos})")
                }
                UserField::Cc => {
                    format!(
                        "EXISTS (SELECT 1 FROM ticket_cc tc \
                         JOIN users u ON tc.user_id = u.id \
                         WHERE tc.ticket_id = t.id AND u.login = ?{pos})"
                    )
                }
            }
        }

        FilterCondition::Status(s) => {
            binds.push(SqlValue::Text(s.clone()));
            let pos = binds.len();
            format!("t.status = ?{pos}")
        }

        FilterCondition::Priority(p) => {
            binds.push(SqlValue::Text(p.clone()));
            let pos = binds.len();
            format!("t.priority = ?{pos}")
        }

        FilterCondition::TicketType(t) => {
            binds.push(SqlValue::Text(t.clone()));
            let pos = binds.len();
            format!("t.type = ?{pos}")
        }

        FilterCondition::Component(c) => {
            // Match component by path prefix: /Component/ matches itself and children.
            binds.push(SqlValue::Text(format!("/{c}/%")));
            let pos = binds.len();
            format!("t.component_id IN (SELECT id FROM components WHERE path LIKE ?{pos})")
        }

        FilterCondition::Milestone(m) => {
            binds.push(SqlValue::Text(m.clone()));
            let pos = binds.len();
            format!(
                "EXISTS (SELECT 1 FROM ticket_milestones tm \
                 JOIN milestones m ON tm.milestone_id = m.id \
                 WHERE tm.ticket_id = t.id AND m.name = ?{pos})"
            )
        }

        FilterCondition::Slug(_) => {
            return Err(BuildError::Unsupported(
                "slug filter requires slug column (task 2.14)".to_string(),
            ));
        }

        FilterCondition::Is(cond) => match cond {
            IsCondition::Open => "t.status != 'done'".to_string(),
            IsCondition::Closed => "t.status = 'done'".to_string(),
        },

        FilterCondition::DateComparison { field, op, value } => {
            let col = match field {
                DateField::Created => "t.created_at",
                DateField::Updated => "t.updated_at",
            };
            let sql_op = comparison_op_sql(op);
            binds.push(SqlValue::Text(format!("{value}T00:00:00Z")));
            let pos = binds.len();
            format!("{col} {sql_op} ?{pos}")
        }

        FilterCondition::EstimationComparison { op, hours } => {
            let sql_op = comparison_op_sql(op);
            binds.push(SqlValue::Float(*hours));
            let pos = binds.len();
            format!("t.estimation_hours {sql_op} ?{pos}")
        }

        FilterCondition::Has(field) => match field {
            HasField::Estimation => "t.estimation_hours IS NOT NULL".to_string(),
            HasField::Milestone => {
                "EXISTS (SELECT 1 FROM ticket_milestones WHERE ticket_id = t.id)".to_string()
            }
        },
    };

    Ok(fragment)
}

fn comparison_op_sql(op: &ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Gt => ">",
        ComparisonOp::Lt => "<",
        ComparisonOp::Gte => ">=",
        ComparisonOp::Lte => "<=",
    }
}

// ---------------------------------------------------------------------------
// Full SQL assembly
// ---------------------------------------------------------------------------

/// Assembles a full SQL query for FTS mode (offset pagination, BM25 ranking).
fn build_fts_query(
    where_parts: &[String],
    binds: &[SqlValue],
    _fts_match: Option<&str>,
    page_size: i64,
    offset: Option<i64>,
) -> Result<BuiltQuery, BuildError> {
    // The FTS MATCH bind is always ?1.
    let mut where_clause = String::from("fts MATCH ?1");

    for part in where_parts {
        where_clause.push_str(" AND ");
        where_clause.push_str(part);
    }

    let sql = format!(
        "SELECT t.* FROM tickets t \
         JOIN tickets_fts fts ON fts.rowid = t.id \
         WHERE {where_clause} \
         ORDER BY bm25(fts, 10.0, 1.0) \
         LIMIT ?{limit_pos} OFFSET ?{offset_pos}",
        limit_pos = binds.len() + 1,
        offset_pos = binds.len() + 2,
    );

    let count_sql = format!(
        "SELECT COUNT(*) FROM tickets t \
         JOIN tickets_fts fts ON fts.rowid = t.id \
         WHERE {where_clause}"
    );

    let mut all_binds = binds.to_vec();
    let count_binds = all_binds.clone();
    all_binds.push(SqlValue::Int(page_size));
    all_binds.push(SqlValue::Int(offset.unwrap_or(0)));

    Ok(BuiltQuery {
        sql,
        binds: all_binds,
        has_fts: true,
        count_sql: Some(count_sql),
        count_binds,
    })
}

/// Assembles a full SQL query for cursor pagination (no FTS).
fn build_cursor_query(
    where_parts: &[String],
    mut binds: Vec<SqlValue>,
    page_size: i64,
    cursor: Option<(&str, i64)>,
) -> BuiltQuery {
    let mut conditions: Vec<String> = where_parts.to_vec();

    if let Some((updated_at, id)) = cursor {
        binds.push(SqlValue::Text(updated_at.to_string()));
        let ts_pos = binds.len();
        binds.push(SqlValue::Int(id));
        let id_pos = binds.len();
        conditions.push(format!("(t.updated_at, t.id) < (?{ts_pos}, ?{id_pos})"));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Fetch page_size + 1 to detect has_more.
    binds.push(SqlValue::Int(page_size + 1));
    let limit_pos = binds.len();

    let sql = format!(
        "SELECT t.* FROM tickets t \
         {where_clause} \
         ORDER BY t.updated_at DESC, t.id DESC \
         LIMIT ?{limit_pos}"
    );

    BuiltQuery {
        sql,
        binds,
        has_fts: false,
        count_sql: None,
        count_binds: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::parser;

    /// Helper: build and unwrap.
    fn build(input: &str) -> BuiltQuery {
        let q = parser::parse(input);
        build_search_query(&q, 25, None, None).unwrap()
    }

    // ---- Individual filter SQL fragments ----------------------------------

    #[test]
    fn filter_owner_sql() {
        let bq = build("owner:alex");
        assert!(
            bq.sql
                .contains("t.owner_id IN (SELECT id FROM users WHERE login = ?1)")
        );
        assert_eq!(bq.binds[0], SqlValue::Text("alex".into()));
        assert!(!bq.has_fts);
    }

    #[test]
    fn filter_cc_sql() {
        let bq = build("cc:maria");
        assert!(bq.sql.contains("EXISTS (SELECT 1 FROM ticket_cc tc"));
        assert!(bq.sql.contains("u.login = ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("maria".into()));
    }

    #[test]
    fn filter_status_sql() {
        let bq = build("status:new");
        assert!(bq.sql.contains("t.status = ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("new".into()));
    }

    #[test]
    fn filter_priority_sql() {
        let bq = build("priority:P0");
        assert!(bq.sql.contains("t.priority = ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("P0".into()));
    }

    #[test]
    fn filter_type_sql() {
        let bq = build("type:bug");
        assert!(bq.sql.contains("t.type = ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("bug".into()));
    }

    #[test]
    fn filter_component_sql() {
        let bq = build("component:Platform/DNS");
        assert!(bq.sql.contains("path LIKE ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("/Platform/DNS/%".into()));
    }

    #[test]
    fn filter_milestone_sql() {
        let bq = build("milestone:v2.0");
        assert!(
            bq.sql
                .contains("EXISTS (SELECT 1 FROM ticket_milestones tm")
        );
        assert!(bq.sql.contains("m.name = ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("v2.0".into()));
    }

    #[test]
    fn filter_is_open_sql() {
        let bq = build("is:open");
        assert!(bq.sql.contains("t.status != 'done'"));
    }

    #[test]
    fn filter_is_closed_sql() {
        let bq = build("is:closed");
        assert!(bq.sql.contains("t.status = 'done'"));
    }

    #[test]
    fn filter_date_created_gt() {
        let bq = build("created:>2026-01-15");
        assert!(bq.sql.contains("t.created_at > ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("2026-01-15T00:00:00Z".into()));
    }

    #[test]
    fn filter_date_updated_lte() {
        let bq = build("updated:<=2026-03-01");
        assert!(bq.sql.contains("t.updated_at <= ?1"));
        assert_eq!(bq.binds[0], SqlValue::Text("2026-03-01T00:00:00Z".into()));
    }

    #[test]
    fn filter_estimation_gt() {
        let bq = build("estimation:>2d");
        assert!(bq.sql.contains("t.estimation_hours > ?1"));
        assert_eq!(bq.binds[0], SqlValue::Float(16.0));
    }

    #[test]
    fn filter_has_estimation_sql() {
        let bq = build("has:estimation");
        assert!(bq.sql.contains("t.estimation_hours IS NOT NULL"));
    }

    #[test]
    fn filter_has_milestone_sql() {
        let bq = build("has:milestone");
        assert!(
            bq.sql
                .contains("EXISTS (SELECT 1 FROM ticket_milestones WHERE ticket_id = t.id)")
        );
    }

    // ---- Negation ---------------------------------------------------------

    #[test]
    fn negated_filter_sql() {
        let bq = build("-status:done");
        assert!(bq.sql.contains("NOT (t.status = ?1)"));
        assert_eq!(bq.binds[0], SqlValue::Text("done".into()));
    }

    // ---- OR groups --------------------------------------------------------

    #[test]
    fn or_group_sql() {
        let bq = build("status:new OR status:in_progress");
        assert!(bq.sql.contains("(t.status = ?1 OR t.status = ?2)"));
        assert_eq!(bq.binds[0], SqlValue::Text("new".into()));
        assert_eq!(bq.binds[1], SqlValue::Text("in_progress".into()));
    }

    // ---- FTS match --------------------------------------------------------

    #[test]
    fn fts_match_simple() {
        let terms = vec![
            TextTerm {
                negated: false,
                text: "crash".into(),
                is_phrase: false,
            },
            TextTerm {
                negated: false,
                text: "startup".into(),
                is_phrase: false,
            },
        ];
        assert_eq!(build_fts_match(&terms), "crash startup");
    }

    #[test]
    fn fts_match_phrase() {
        let terms = vec![TextTerm {
            negated: false,
            text: "stack overflow".into(),
            is_phrase: true,
        }];
        assert_eq!(build_fts_match(&terms), "\"stack overflow\"");
    }

    #[test]
    fn fts_match_short_term() {
        let terms = vec![TextTerm {
            negated: false,
            text: "ab".into(),
            is_phrase: false,
        }];
        assert_eq!(build_fts_match(&terms), "ab*");
    }

    #[test]
    fn fts_match_negated() {
        let terms = vec![TextTerm {
            negated: true,
            text: "crash".into(),
            is_phrase: false,
        }];
        assert_eq!(build_fts_match(&terms), "NOT crash");
    }

    // ---- Combined filter + FTS --------------------------------------------

    #[test]
    fn combined_fts_with_filters() {
        let bq = build("status:new crash on startup");
        assert!(bq.has_fts);
        assert!(bq.sql.contains("JOIN tickets_fts fts ON fts.rowid = t.id"));
        assert!(bq.sql.contains("fts MATCH ?1"));
        assert!(bq.sql.contains("t.status = ?2"));
        assert!(bq.sql.contains("ORDER BY bm25(fts, 10.0, 1.0)"));
        assert!(bq.count_sql.is_some());
    }

    // ---- Cursor pagination (no FTS) ---------------------------------------

    #[test]
    fn cursor_pagination() {
        let q = parser::parse("status:new");
        let bq = build_search_query(&q, 25, None, Some(("2026-01-01T00:00:00.000Z", 42))).unwrap();
        assert!(!bq.has_fts);
        assert!(bq.sql.contains("(t.updated_at, t.id) < (?2, ?3)"));
        assert!(bq.sql.contains("ORDER BY t.updated_at DESC, t.id DESC"));
        // page_size + 1 for has_more detection
        assert_eq!(bq.binds.last(), Some(&SqlValue::Int(26)));
    }

    // ---- Slug → unsupported error -----------------------------------------

    #[test]
    fn slug_filter_unsupported() {
        let q = parser::parse("slug:MAP");
        let result = build_search_query(&q, 25, None, None);
        assert!(matches!(result, Err(BuildError::Unsupported(_))));
    }

    // ---- Empty query → no WHERE clauses -----------------------------------

    #[test]
    fn empty_query() {
        let bq = build("");
        assert!(!bq.has_fts);
        assert!(!bq.sql.contains("WHERE"));
        assert!(bq.sql.contains("SELECT t.* FROM tickets t"));
    }
}
