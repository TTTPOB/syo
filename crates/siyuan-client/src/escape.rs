//! Helpers for safely composing SQLite queries against the SiYuan kernel.
//!
//! The kernel's `/api/query/sql` endpoint accepts a raw SQL string, so all
//! interpolation happens client-side. These helpers exist to keep that
//! interpolation honest: callers must escape both single quotes (so the
//! string literal closes where intended) and LIKE meta-characters (`%`,
//! `_`, `\`) so a substring search behaves as a substring search.

/// Hard cap applied to user-supplied `LIMIT` values in search tools.
///
/// The SiYuan kernel will happily accept absurdly large limits; capping
/// here protects both the kernel and the agent (large result sets blow
/// past the model's context window). 1000 is generous enough to cover
/// realistic browse-and-narrow workflows while keeping a single page of
/// results under any reasonable token budget.
pub const MAX_SEARCH_LIMIT: u64 = 1000;

/// Escape a substring for safe inclusion in a SQLite `LIKE` pattern.
///
/// The returned string is meant to be wrapped in `'%...%'` and used with an
/// explicit `ESCAPE '\\'` clause, e.g.:
///
/// ```text
/// markdown LIKE '%foo\_bar%' ESCAPE '\'
/// ```
///
/// Order matters: backslash must be escaped first, otherwise the
/// backslashes we introduce when escaping `%` and `_` would themselves
/// be doubled.
///
/// Single quotes are also doubled so the result is safe to drop into a
/// string literal without further processing.
pub fn escape_sql_like(s: &str) -> String {
    // Capacity guess: most strings need no escaping; a few need a handful.
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
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
        assert_eq!(escape_sql_like("hello world"), "hello world");
    }

    #[test]
    fn single_quote_is_doubled() {
        assert_eq!(escape_sql_like("it's"), "it''s");
    }

    #[test]
    fn backslash_is_doubled_first() {
        // The single backslash in input must become exactly two backslashes
        // in output, not four (which would happen if `\` were escaped after
        // `%`/`_`, since the escape sequences themselves contain backslashes).
        assert_eq!(escape_sql_like(r"a\b"), r"a\\b");
    }

    #[test]
    fn percent_is_escaped() {
        assert_eq!(escape_sql_like("100%"), r"100\%");
    }

    #[test]
    fn underscore_is_escaped() {
        assert_eq!(escape_sql_like("foo_bar"), r"foo\_bar");
    }

    #[test]
    fn combined_meta_characters() {
        // Order: a ' \\ % _ z  ->  a '' \\\\ \\% \\_ z
        assert_eq!(escape_sql_like(r"a'\%_z"), r"a''\\\%\_z");
    }

    #[test]
    fn empty_string_is_empty() {
        assert_eq!(escape_sql_like(""), "");
    }
}
