//! AST-level read-only validation for raw SQL statements.
//!
//! # Why an AST check, not a lexical one
//!
//! Earlier iterations relied on `head.starts_with("select" | "with")`. That is
//! easy to bypass (`WITH cte AS (...) DELETE FROM ...` passes the prefix test
//! but is a write) and easy to false-reject (`-- comment\nSELECT` looked like
//! a non-keyword start).
//!
//! The kernel does NOT make up for that with its own filter. SiYuan security
//! advisories GHSA-jqwg-75qf-vmf9 and GHSA-j7wh-x834-p3r7 document that the
//! `/api/query/sql` endpoint historically accepted write SQL, and the current
//! kernel only gates it via admin-role + non-publish-mode middleware — neither
//! is a SQL-level filter. Whatever bypasses our client-side check CAN execute.
//!
//! Therefore: parse with `sqlparser-rs` (SQLite dialect), require exactly one
//! statement, accept only `Query` or `Explain { statement: Query }`. Everything
//! else (INSERT/UPDATE/DELETE/DDL/PRAGMA/ATTACH/DETACH/multi-statement) is
//! rejected before any kernel round trip.

use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::{Parser, ParserError};

/// Categorised guard failure. Each variant carries enough context for the
/// caller to emit a useful CLI / MCP error message.
#[derive(Debug, thiserror::Error)]
pub enum GuardError {
    #[error("statement must not be empty or whitespace-only")]
    Empty,

    #[error("only a single statement is allowed; parsed {count}")]
    MultiStatement { count: usize },

    #[error("only read-only queries are allowed; got {kind}")]
    NonReadOnly { kind: String },

    #[error("could not parse as SQLite SQL: {0}")]
    Parse(String),
}

/// Validate that `stmt` is a single read-only SQL statement.
///
/// Accepts:
/// - `SELECT ...`
/// - `WITH ... SELECT ...` (and `WITH ... VALUES`, `WITH ... UNION ...`)
/// - `VALUES (...)` (a query that returns inline rows)
/// - `EXPLAIN <select>` and `EXPLAIN QUERY PLAN <select>` (when the inner
///   statement is itself a query)
///
/// Rejects (with `NonReadOnly`):
/// - INSERT / UPDATE / DELETE / MERGE / REPLACE
/// - CREATE / DROP / ALTER / TRUNCATE / RENAME
/// - PRAGMA / ATTACH / DETACH / VACUUM / ANALYZE / REINDEX
/// - EXPLAIN of any non-query statement
/// - Anything sqlparser-rs surfaces under a different `Statement` variant
///
/// Rejects (with other variants):
/// - empty / whitespace input → `Empty`
/// - `SELECT 1; DROP TABLE foo` and similar → `MultiStatement`
/// - syntactic garbage → `Parse`
pub fn validate_read_only(stmt: &str) -> Result<(), GuardError> {
    if stmt.trim().is_empty() {
        return Err(GuardError::Empty);
    }
    let dialect = SQLiteDialect {};
    let statements = Parser::parse_sql(&dialect, stmt).map_err(|e: ParserError| {
        GuardError::Parse(match e {
            ParserError::TokenizerError(s) | ParserError::ParserError(s) => s,
            other => other.to_string(),
        })
    })?;

    match statements.len() {
        0 => Err(GuardError::Empty),
        1 => is_read_only(&statements[0]),
        n => Err(GuardError::MultiStatement { count: n }),
    }
}

fn is_read_only(stmt: &Statement) -> Result<(), GuardError> {
    match stmt {
        // sqlparser-rs models `WITH cte AS (...) DELETE/UPDATE/INSERT ...` as
        // a `Statement::Query` whose `body` is `SetExpr::Delete/Update/Insert`.
        // The top-level kind is therefore not enough — we have to recurse
        // into the body to catch CTE-prefixed writes.
        Statement::Query(q) => {
            check_query_body(&q.body).map_err(|kind| GuardError::NonReadOnly { kind })
        }
        Statement::Explain {
            statement: inner, ..
        } => match &**inner {
            Statement::Query(q) => {
                check_query_body(&q.body).map_err(|kind| GuardError::NonReadOnly {
                    kind: format!("EXPLAIN {kind}"),
                })
            }
            other => Err(GuardError::NonReadOnly {
                kind: format!("EXPLAIN {}", kind_label(other)),
            }),
        },
        other => Err(GuardError::NonReadOnly {
            kind: kind_label(other),
        }),
    }
}

/// Validate a `SetExpr` (the body of a `Query`). Read-only set expressions
/// are SELECT, VALUES, TABLE, and set operations / parenthesised queries
/// composed of them. Anything else — including the CTE-prefixed
/// INSERT/UPDATE/DELETE that sqlparser models under `Statement::Query` —
/// must be rejected.
///
/// Returns `Err(<short label>)` so the caller can wrap it into the
/// appropriate `GuardError` variant. Default-deny: unknown variants (added
/// in future sqlparser-rs releases) are treated as writes until proven
/// otherwise.
fn check_query_body(body: &SetExpr) -> Result<(), String> {
    match body {
        SetExpr::Select(_) | SetExpr::Values(_) | SetExpr::Table(_) => Ok(()),
        SetExpr::Query(inner) => check_query_body(&inner.body),
        SetExpr::SetOperation { left, right, .. } => {
            check_query_body(left)?;
            check_query_body(right)
        }
        SetExpr::Insert(_) => Err("INSERT".into()),
        SetExpr::Update(_) => Err("UPDATE".into()),
        SetExpr::Delete(_) => Err("DELETE".into()),
    }
}

/// Short, human-readable label for a `Statement` variant. We deliberately do
/// NOT enumerate every variant — `sqlparser-rs` adds new ones over time, and
/// the catch-all keeps this guard correct on upgrade. Common writes get a
/// crisp label so the error message is actionable.
fn kind_label(stmt: &Statement) -> String {
    match stmt {
        Statement::Insert(_) => "INSERT".into(),
        Statement::Update { .. } => "UPDATE".into(),
        Statement::Delete(_) => "DELETE".into(),
        Statement::CreateTable(_) => "CREATE TABLE".into(),
        Statement::CreateIndex(_) => "CREATE INDEX".into(),
        Statement::CreateView { .. } => "CREATE VIEW".into(),
        Statement::Drop { .. } => "DROP".into(),
        Statement::AlterTable { .. } => "ALTER TABLE".into(),
        Statement::Truncate { .. } => "TRUNCATE".into(),
        Statement::Pragma { .. } => "PRAGMA".into(),
        Statement::AttachDatabase { .. } => "ATTACH".into(),
        Statement::Explain { statement, .. } => {
            format!("EXPLAIN {}", kind_label(statement))
        }
        // Fall-through: emit the variant's debug discriminant. This is uglier
        // than a curated label but stays correct when sqlparser adds variants
        // we haven't seen.
        other => {
            let raw = format!("{other:?}");
            // Take the first identifier-shaped token, e.g. `Truncate { .. }`
            // → `Truncate`. The full Debug rendering can be a multi-line
            // payload that would dominate the error message.
            raw.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
                .to_ascii_uppercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(stmt: &str) {
        validate_read_only(stmt).unwrap_or_else(|e| panic!("expected ok, got {e:?}: {stmt}"));
    }

    fn err(stmt: &str) -> GuardError {
        validate_read_only(stmt).expect_err(&format!("expected err: {stmt}"))
    }

    #[test]
    fn accepts_plain_select() {
        ok("SELECT 1");
    }

    #[test]
    fn accepts_select_with_columns_and_table() {
        ok("SELECT id, hpath FROM blocks WHERE type = 'd' LIMIT 5");
    }

    #[test]
    fn accepts_with_select() {
        ok("WITH x AS (SELECT id FROM blocks) SELECT * FROM x");
    }

    #[test]
    fn accepts_recursive_cte() {
        ok("WITH RECURSIVE n(i) AS (VALUES (1) UNION SELECT i+1 FROM n WHERE i<5) SELECT i FROM n");
    }

    #[test]
    fn accepts_values_query() {
        ok("VALUES (1), (2), (3)");
    }

    #[test]
    fn accepts_union() {
        ok("SELECT 1 UNION SELECT 2");
    }

    #[test]
    fn accepts_explain_select() {
        ok("EXPLAIN SELECT 1");
    }

    #[test]
    fn accepts_explain_query_plan() {
        ok("EXPLAIN QUERY PLAN SELECT id FROM blocks");
    }

    #[test]
    fn accepts_leading_line_comment() {
        ok("-- pinned for the daily-note query\nSELECT id FROM blocks");
    }

    #[test]
    fn accepts_leading_block_comment() {
        ok("/* daily-note query */ SELECT id FROM blocks");
    }

    #[test]
    fn accepts_trailing_semicolon() {
        // Single-statement input with a stray trailing `;` parses as one
        // Statement, so the multi-statement check must not fire.
        ok("SELECT 1;");
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(err(""), GuardError::Empty));
    }

    #[test]
    fn rejects_whitespace_only() {
        assert!(matches!(err("   \n\t "), GuardError::Empty));
    }

    #[test]
    fn rejects_comment_only() {
        // A statement that contains only comments parses to zero statements
        // — the parser eats the comment and yields an empty Vec. The guard
        // surfaces that as Empty, not Parse, so the message stays user-
        // facing.
        assert!(matches!(err("-- nothing here\n"), GuardError::Empty));
    }

    #[test]
    fn rejects_insert() {
        let GuardError::NonReadOnly { kind } = err("INSERT INTO blocks (id) VALUES ('x')") else {
            panic!("wrong variant");
        };
        assert!(kind.contains("INSERT"), "got: {kind}");
    }

    #[test]
    fn rejects_update() {
        let GuardError::NonReadOnly { kind } = err("UPDATE blocks SET name = 'x'") else {
            panic!("wrong variant");
        };
        assert!(kind.contains("UPDATE"), "got: {kind}");
    }

    #[test]
    fn rejects_delete() {
        let GuardError::NonReadOnly { kind } = err("DELETE FROM blocks WHERE 1=1") else {
            panic!("wrong variant");
        };
        assert!(kind.contains("DELETE"), "got: {kind}");
    }

    #[test]
    fn rejects_drop_table() {
        let GuardError::NonReadOnly { kind } = err("DROP TABLE blocks") else {
            panic!("wrong variant");
        };
        assert!(kind.contains("DROP"), "got: {kind}");
    }

    #[test]
    fn rejects_with_tail_delete() {
        // The whole point of upgrading to AST validation: `WITH cte AS (...)
        // DELETE FROM ...` passes a leading-keyword test (`with`) but is a
        // write. The AST sees the DELETE under the WITH and rejects.
        let GuardError::NonReadOnly { kind } = err(
            "WITH x AS (SELECT id FROM blocks) DELETE FROM blocks WHERE id IN (SELECT id FROM x)",
        ) else {
            panic!("wrong variant");
        };
        assert!(kind.contains("DELETE"), "got: {kind}");
    }

    #[test]
    fn rejects_pragma_write() {
        // Writable-side PRAGMAs would change SQLite session state if they
        // executed. We reject all PRAGMAs uniformly because read-side ones
        // (`PRAGMA optimize`) can also have side effects, and the tradeoff
        // of "occasional false reject for a debugging convenience" beats the
        // alternative.
        let GuardError::NonReadOnly { kind } = err("PRAGMA writable_schema = 1") else {
            panic!("wrong variant");
        };
        assert!(kind.contains("PRAGMA"), "got: {kind}");
    }

    #[test]
    fn rejects_pragma_read() {
        // `PRAGMA encoding` (no value, no parens) parses cleanly under
        // sqlparser-rs's SQLiteDialect — pin the read variant explicitly so a
        // future sqlparser refactor that re-buckets PRAGMAs cannot silently
        // turn this into a Parse-fallthrough.
        let GuardError::NonReadOnly { kind } = err("PRAGMA encoding") else {
            panic!("wrong variant: {:?}", err("PRAGMA encoding"));
        };
        assert!(kind.contains("PRAGMA"), "got: {kind}");
    }

    #[test]
    fn rejects_attach() {
        let GuardError::NonReadOnly { kind } = err("ATTACH DATABASE 'evil.db' AS evil") else {
            panic!("wrong variant");
        };
        assert!(kind.contains("ATTACH"), "got: {kind}");
    }

    #[test]
    fn rejects_multi_statement_select_drop() {
        // `SELECT 1; DROP TABLE blocks` parses as TWO statements; the first
        // is a Query, the second is a Drop. Reject before we forward the
        // first half to the kernel and lose track of the second.
        let GuardError::MultiStatement { count } = err("SELECT 1; DROP TABLE blocks") else {
            panic!("wrong variant");
        };
        assert_eq!(count, 2);
    }

    #[test]
    fn rejects_explain_of_write() {
        // `EXPLAIN INSERT ...` is a debugging affordance but the inner
        // statement is still write-shaped. We require the wrapped Statement
        // to be a Query.
        let GuardError::NonReadOnly { kind } = err("EXPLAIN INSERT INTO blocks (id) VALUES ('x')")
        else {
            panic!("wrong variant");
        };
        assert!(kind.contains("EXPLAIN"), "got: {kind}");
        assert!(kind.contains("INSERT"), "got: {kind}");
    }

    #[test]
    fn rejects_garbage_with_parse_error() {
        // Random garbage surfaces as `Parse`, not `NonReadOnly`. The message
        // includes the parser's diagnostic so the user can see where the
        // parse failed.
        let GuardError::Parse(_) = err("this is not sql at all") else {
            panic!("wrong variant: {:?}", err("this is not sql at all"));
        };
    }
}
