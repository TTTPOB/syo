use anyhow::{Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;

#[derive(Args, Debug)]
pub struct SqlArgs {
    /// Read-only SQL SELECT statement to run against the SiYuan SQLite store.
    ///
    /// Power-tool escape hatch — prefer `siyuan search`, `siyuan tag`,
    /// `siyuan graph` etc. when they cover the use case.
    ///
    /// Read-only: INSERT/UPDATE/DELETE/DDL are rejected by the kernel. In
    /// read-only / publish mode the endpoint itself is disabled and you'll
    /// get a typed `SqlUnavailable` error.
    ///
    /// NOT parameterised: single quotes and LIKE meta-characters in your SQL
    /// must be escaped by you — there is no auto-escaping. Treat the value
    /// as literal SQL text.
    ///
    /// The SQL index lags writes by ~100–500 ms; rows you just inserted may
    /// not show up immediately.
    ///
    /// `LIMIT` belongs inside your SQL — there is no `--limit` flag.
    #[arg(long)]
    pub stmt: String,
}

pub async fn run(client: &SiyuanClient, args: SqlArgs) -> Result<()> {
    // Reject blank input client-side. The kernel would also error, but failing
    // early avoids a useless round trip and produces a clearer message.
    if args.stmt.trim().is_empty() {
        bail!("--stmt must not be empty");
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
}
