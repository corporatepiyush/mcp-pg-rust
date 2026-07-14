use anyhow::Result;
use clap::Parser;
use fake::Fake;
use fake::faker::company::en::CompanyName;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::{FirstName, LastName, Name};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::info;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value = "3000")]
    port: u16,
    #[arg(long, default_value = "1000000")]
    target_rows: u64,
    #[arg(long, default_value = "16")]
    concurrency: usize,
}

struct Generator {
    host: String,
    port: u16,
    total: Arc<AtomicU64>,
}

impl Generator {
    fn new(h: String, p: u16) -> Self {
        Self {
            host: h,
            port: p,
            total: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn call(&self, n: &str, a: Value) -> Result<Value> {
        let mut s = TcpStream::connect(format!("{}:{}", self.host, self.port)).await?;
        let r =
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":n,"arguments":a}});
        let m = serde_json::to_vec(&r)?;
        s.write_all(&m).await?;
        s.write_all(
            b"
",
        )
        .await?;
        let mut b = vec![0; 65536];
        let n = s.read(&mut b).await?;
        let rs: Value = serde_json::from_slice(&b[..n])?;
        if let Some(e) = rs.get("error") {
            return Err(anyhow::anyhow!("Error: {:?}", e));
        }
        Ok(rs["result"].clone())
    }

    async fn create_org(&self) -> Result<i64> {
        let res = self.call("batch_insert", json!({"table":"organizations", "columns":["name","domain","revenue","employee_count","country"], "rows":[[CompanyName().fake::<String>(), SafeEmail().fake::<String>(), 1000.0, 10, "US"]], "returning":"id"})).await?;
        self.total.fetch_add(1, Ordering::Relaxed);
        Ok(res["inserted_ids"][0].as_i64().unwrap_or(0))
    }
}

impl Generator {
    async fn create_user(&self, oid: i64) -> Result<i64> {
        let res = self.call("batch_insert", json!({"table":"users", "columns":["org_id","email","username","first_name","last_name","role"], "rows":[[oid, SafeEmail().fake::<String>(), "user", FirstName().fake::<String>(), LastName().fake::<String>(), "member"]], "returning":"id"})).await?;
        self.total.fetch_add(1, Ordering::Relaxed);
        Ok(res["inserted_ids"][0].as_i64().unwrap_or(0))
    }

    async fn create_project(&self, oid: i64) -> Result<i64> {
        let res = self.call("batch_insert", json!({"table":"projects", "columns":["org_id","name","status","start_year"], "rows":[[oid, Name().fake::<String>(), "active", 2024]], "returning":"id"})).await?;
        self.total.fetch_add(1, Ordering::Relaxed);
        Ok(res["inserted_ids"][0].as_i64().unwrap_or(0))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let a = Args::parse();
    let g = Arc::new(Generator::new(a.host.clone(), a.port));
    info!("Starting transactional load generator...");

    let mut h = vec![];
    for _ in 0..a.concurrency {
        let g = g.clone();
        let t = a.target_rows;
        h.push(tokio::spawn(async move {
            while g.total.load(Ordering::Relaxed) < t {
                if let Ok(oid) = g.create_org().await {
                    let _ = g.create_user(oid).await;
                    let _ = g.create_project(oid).await;
                }
            }
        }));
    }

    for handle in h {
        let _ = handle.await;
    }
    info!("Done: {} records", g.total.load(Ordering::Relaxed));
    Ok(())
}
