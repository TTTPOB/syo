//! Helpers for safely composing SQL queries against the SiYuan kernel.
//!
//! The kernel's `/api/query/sql` endpoint accepts a raw SQL string, so all
//! interpolation happens client-side. Single quotes are doubled for SQL
//! string-literal safety.
//!
//! Note: the SiYuan kernel's SQL engine does **not** support `ESCAPE '\'`
//! in `LIKE` patterns. The `%` and `_` characters in user input therefore
//! always behave as LIKE wildcards -- there is no way to escape them
//! server-side. This is a known engine limitation.

/// Hard cap applied to user-supplied `LIMIT` values in search tools.
///
/// The SiYuan kernel will happily accept absurdly large limits; capping
/// here protects both the kernel and the agent (large result sets blow
/// past the model's context window). 1000 is generous enough to cover
/// realistic browse-and-narrow workflows while keeping a single page of
/// results under any reasonable token budget.
pub const MAX_SEARCH_LIMIT: u64 = 1000;

/// Escape a value for safe inclusion in a SQL string literal.
///
/// Only single quotes are doubled (`'` → `''`). The SiYuan kernel's SQL
/// engine does not support `ESCAPE '\'`, so LIKE meta-characters (`%`, `_`)
/// cannot be escaped -- they will behave as wildcards in `LIKE` patterns.
/// Callers that need exact-match semantics should use `=` instead of `LIKE`.
///
/// A backslash (`\`) in the input is left as-is; without `ESCAPE` support
/// it carries no special meaning in the kernel's LIKE engine.
pub fn escape_sql_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '\'' => out.push_str("''"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_unchanged() {
        assert_eq!(escape_sql_string("hello world"), "hello world");
    }

    #[test]
    fn single_quote_is_doubled() {
        assert_eq!(escape_sql_string("it's"), "it''s");
    }

    #[test]
    fn backslash_unchanged() {
        // Without ESCAPE support, backslash carries no special meaning.
        assert_eq!(escape_sql_string(r"a\b"), r"a\b");
    }

    #[test]
    fn percent_unchanged() {
        // % cannot be escaped -- it behaves as a LIKE wildcard.
        assert_eq!(escape_sql_string("100%"), "100%");
    }

    #[test]
    fn underscore_unchanged() {
        // _ cannot be escaped -- it behaves as a LIKE wildcard.
        assert_eq!(escape_sql_string("foo_bar"), "foo_bar");
    }

    #[test]
    fn combined_meta_characters() {
        // Only single quotes are doubled.
        assert_eq!(escape_sql_string(r"a'\%_z"), r"a''\%_z");
    }

    #[test]
    fn empty_string_is_empty() {
        assert_eq!(escape_sql_string(""), "");
    }
}
