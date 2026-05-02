use anyhow::{Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::sql_guard;

/// Run a single read-only SQL statement against the SiYuan SQLite store.
///
/// Sibling commands: prefer `siyuan search text`, `siyuan search blocks`,
/// `siyuan tag search`, or `siyuan graph neighborhood` when they cover the
/// use case. Reach for sql ONLY when those do not (joins, aggregates, or
/// access to internal tables like `refs`, `attributes`, `spans`).
///
/// READ-ONLY ONLY. The statement is parsed locally with sqlparser-rs
/// (SQLite dialect, single-statement) and rejected if the AST is anything
/// other than a `SELECT` / `WITH ... SELECT` / `VALUES` / `EXPLAIN <select>`
/// node. `INSERT`/`UPDATE`/`DELETE`/`CREATE`/`DROP`/`ALTER`/`PRAGMA`/
/// `ATTACH`/`DETACH` and multi-statement input (`SELECT ...; DROP ...`)
/// are all rejected client-side without a kernel round trip. CTE-tail
/// writes (`WITH cte AS (...) DELETE ...`, `... UPDATE ...`,
/// `... INSERT ...`) are also rejected — the AST check sees the write
/// underneath the CTE.
///
/// Do NOT assume the kernel itself enforces read-only at the SQL level.
/// SiYuan security advisories GHSA-jqwg-75qf-vmf9 and GHSA-j7wh-x834-p3r7
/// document that `/api/query/sql` historically accepted write SQL. The
/// current kernel only gates the endpoint via admin-role + non-publish-
/// mode middleware — neither is a SQL-level filter — so any write that
/// bypassed our client-side guard CAN execute.
///
/// Inputs:
///   --stmt (required): a single read-only statement. Whitespace-only
///     input is rejected. Leading line (`-- ...`) and block (`/* ... */`)
///     comments are stripped by the parser. In read-only / publish mode
///     the endpoint is disabled entirely (you'll get `SqlUnavailable`).
///
/// Critical caveats:
///   * No parameterisation. Single quotes inside string literals must be
///     doubled (`'O''Brien'`). The SiYuan SQL engine does **not** support
///     `ESCAPE` for `LIKE`, so `%` and `_` always behave as wildcards.
///     `LIMIT` belongs in
///     your SQL — there is no `--limit` flag.
///   * Use SQLite syntax. The kernel parses through a MySQL grammar as
///     a fallback and may re-serialise the AST before execution, so
///     MySQL-flavoured constructs (`CONCAT(...)`, `NOW()`, backtick
///     identifiers, `LIMIT m, n`, `IF(c,a,b)`) may parse but fail or
///     misbehave at execution. Stick to: `||` for concat,
///     `datetime('now')`, `"col"`/`[col]` quoting, `LIMIT n OFFSET m`,
///     `iif(c,a,b)` or `CASE`.
///   * `REGEXP` is supported via a kernel-registered custom function;
///     other non-stock SQLite functions are not.
///
/// Output is the raw row array as pretty JSON (each row is an object whose
/// keys are column names). The SQL index lags writes by ~100-500 ms — rows
/// the user just inserted may not show up immediately even though the
/// kernel has them.
///
/// Example:
///   in:  --stmt "SELECT id, hpath FROM blocks WHERE box = '20260501000000-nb00001' AND type = 'd' LIMIT 5"
///   out: [
///          {"id":"20260501090000-doc0001","hpath":"/Plan"},
///          ...
///        ]
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct SqlArgs {
    /// Single read-only SQL statement. Validated locally as `Query` /
    /// `WITH ... Query` / `VALUES` / `EXPLAIN <Query>`; everything else
    /// is rejected before any kernel round trip.
    #[arg(long)]
    pub stmt: String,
}

pub async fn run(client: &SiyuanClient, args: SqlArgs) -> Result<()> {
    // AST-level read-only guard. The kernel does NOT enforce read-only at
    // the SQL level (see security advisories GHSA-jqwg-75qf-vmf9 and
    // GHSA-j7wh-x834-p3r7), so we cannot trust the kernel to catch writes
    // for us — this check is the actual gate. Forward the original `stmt`
    // verbatim to the kernel so quoting / case / whitespace are preserved.
    if let Err(e) = sql_guard::validate_read_only(&args.stmt) {
        bail!("--stmt: {e}");
    }
    let rows = client.sql(&args.stmt).await?;
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_client() -> SiyuanClient {
        // The dummy URL is never reached: empty-stmt validation runs before
        // any HTTP call, so this client is just a placeholder argument.
        SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds")
    }

    #[tokio::test]
    async fn run_rejects_whitespace_stmt() {
        let client = dummy_client();
        let args = SqlArgs { stmt: "   ".into() };
        let err = run(&client, args)
            .await
            .expect_err("whitespace stmt must be rejected");
        assert!(
            err.to_string().contains("--stmt"),
            "error message should reference `--stmt`; got: {err}"
        );
    }

    #[tokio::test]
    async fn run_rejects_empty_stmt() {
        let client = dummy_client();
        let args = SqlArgs { stmt: "".into() };
        let err = run(&client, args)
            .await
            .expect_err("empty stmt must be rejected");
        assert!(
            err.to_string().contains("--stmt"),
            "error message should reference `--stmt`; got: {err}"
        );
    }

    #[tokio::test]
    async fn run_rejects_non_select_stmt() {
        // A `DROP TABLE` would burn a kernel round trip if forwarded — the
        // AST guard rejects it locally. The dummy client points at an
        // unreachable port, so if the guard regresses this test would
        // surface a network error instead of the read-only message; the
        // assertion pins the exact rejection reason.
        let client = dummy_client();
        let args = SqlArgs {
            stmt: "DROP TABLE blocks".into(),
        };
        let err = run(&client, args)
            .await
            .expect_err("non-SELECT stmt must be rejected client-side");
        assert!(
            err.to_string().contains("read-only"),
            "error message should mention the read-only requirement; got: {err}"
        );
    }

    #[tokio::test]
    async fn run_rejects_with_tail_delete() {
        // CTE-tail writes pass the old lexical check (leading `with`) but
        // execute as DELETEs. The AST guard recognises the underlying
        // SetExpr::Delete and rejects them. This test pins that promotion
        // — the lexical guard would have let this through.
        let client = dummy_client();
        let args = SqlArgs {
            stmt: "WITH x AS (SELECT id FROM blocks) DELETE FROM blocks \
                   WHERE id IN (SELECT id FROM x)"
                .into(),
        };
        let err = run(&client, args)
            .await
            .expect_err("WITH-tail DELETE must be rejected by the AST guard");
        assert!(
            err.to_string().contains("DELETE"),
            "error should name the rejected operation; got: {err}"
        );
    }

    #[tokio::test]
    async fn run_rejects_multi_statement() {
        let client = dummy_client();
        let args = SqlArgs {
            stmt: "SELECT 1; DROP TABLE blocks".into(),
        };
        let err = run(&client, args)
            .await
            .expect_err("multi-statement input must be rejected");
        assert!(
            err.to_string().contains("single statement"),
            "error should name the constraint; got: {err}"
        );
    }
}
