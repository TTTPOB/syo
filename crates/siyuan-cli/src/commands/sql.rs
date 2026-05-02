use anyhow::{Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;

/// Run a read-only SQL SELECT directly against the SiYuan SQLite store.
///
/// Sibling commands: prefer `siyuan search text`, `siyuan search blocks`,
/// `siyuan tag search`, or `siyuan graph neighborhood` when they cover the
/// use case. Reach for sql ONLY when those do not (joins, aggregates, or
/// access to internal tables like `refs`, `attributes`, `spans`).
///
/// Inputs:
///   --stmt (required): a single SQL SELECT statement. Whitespace-only
///     input is rejected client-side. A leading-keyword check rejects
///     non-`SELECT`/`WITH` statements (INSERT/UPDATE/DELETE/DDL/PRAGMA)
///     client-side before any kernel round trip; the kernel itself also
///     rejects writes, but we save the trip. In read-only / publish mode
///     the endpoint is disabled entirely (you'll get `SqlUnavailable`).
///
/// Critical caveat: the kernel does NOT parameterise the query. Single
/// quotes inside string literals must be doubled (`'O''Brien'`); LIKE
/// meta-chars (`%`, `_`, `\`) must be escaped by you and paired with an
/// `ESCAPE '\'` clause. Treat the input as literal SQL text — there is no
/// auto-escaping. `LIMIT` belongs in your SQL too; there is no `--limit`
/// flag.
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
    /// Read-only SQL SELECT statement (single statement, no `;`-chaining).
    /// Single quotes and LIKE meta-chars must be escaped by you.
    #[arg(long)]
    pub stmt: String,
}

pub async fn run(client: &SiyuanClient, args: SqlArgs) -> Result<()> {
    // Reject blank input client-side. The kernel would also error, but failing
    // early avoids a useless round trip and produces a clearer message.
    if args.stmt.trim().is_empty() {
        bail!("--stmt must not be empty");
    }
    // Client-side read-only guard: only SELECT and WITH (CTE) statements are
    // accepted. The kernel rejects writes too, but matching here saves a
    // round trip and surfaces a clearer error. We compare a trimmed +
    // lowercased view but still forward the original `stmt` to the kernel.
    let head = args.stmt.trim_start().to_ascii_lowercase();
    if !(head.starts_with("select") || head.starts_with("with")) {
        bail!("--stmt must be a read-only SELECT (or WITH) query");
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
        // A `DROP TABLE` would burn a kernel round trip if forwarded; the
        // client-side leading-keyword guard must reject it locally. The dummy
        // client points at an unreachable port — if the guard regresses, this
        // test would surface a network error instead of the read-only
        // message, so the assertion pins the exact rejection reason.
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
}
