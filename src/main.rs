use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use base64::Engine;
use orbitflare_sdk::{Result, RpcClientBuilder, WsClientBuilder};
use serde_json::Value;
use tokio::sync::mpsc;

struct Holding {
    mint: String,
    decimals: u8,
    amount: u64,
}

enum Update {
    Sol(u64),
    Token { account: String, amount: u64 },
}

#[tokio::main]
async fn main() -> Result<()> {
    let wallet = env::args().nth(1).expect("usage: wallet-ticker <pubkey>");

    let rpc = RpcClientBuilder::new()
        .url("http://ams.rpc.orbitflare.com")
        .commitment("confirmed")
        .build()?;

    let ws = Arc::new(
        WsClientBuilder::new()
            .url("ws://ams.rpc.orbitflare.com")
            .build()
            .await?,
    );

    let mut sol_lamports = rpc.get_balance(&wallet).await?;
    let raw_tokens: Vec<Value> = rpc.get_token_accounts_by_owner(&wallet, None, None).await?;

    let mut holdings: HashMap<String, Holding> = HashMap::new();

    for entry in &raw_tokens {
        let pubkey = entry["pubkey"].as_str().unwrap().to_string();
        let info = &entry["account"]["data"]["parsed"]["info"];

        let mint = info["mint"].as_str().unwrap().to_string();
        let amount: u64 = info["tokenAmount"]["amount"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let decimals = info["tokenAmount"]["decimals"].as_u64().unwrap() as u8;

        holdings.insert(
            pubkey,
            Holding {
                mint,
                decimals,
                amount,
            },
        );
    }

    render(&wallet, sol_lamports, &holdings);

    let (tx, mut rx) = mpsc::channel::<Update>(1024);

    {
        let ws = Arc::clone(&ws);
        let wallet = wallet.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let Ok(mut sub) = ws.account_subscribe(&wallet, "confirmed").await else {
                return;
            };
            while let Some(v) = sub.next().await {
                if let Some(lamports) = v["lamports"].as_u64() {
                    if tx.send(Update::Sol(lamports)).await.is_err() {
                        break;
                    }
                }
            }
        });
    }

    for pubkey in holdings.keys().cloned().collect::<Vec<_>>() {
        let ws = Arc::clone(&ws);
        let tx = tx.clone();
        tokio::spawn(async move {
            let Ok(mut sub) = ws.account_subscribe(&pubkey, "confirmed").await else {
                return;
            };
            while let Some(v) = sub.next().await {
                let Some(data_b64) = v["data"][0].as_str() else {
                    continue;
                };
                let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data_b64) else {
                    continue;
                };
                if bytes.len() < 72 {
                    continue;
                }

                let amount = u64::from_le_bytes(bytes[64..72].try_into().unwrap());
                let update = Update::Token {
                    account: pubkey.clone(),
                    amount,
                };
                if tx.send(update).await.is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);

    while let Some(update) = rx.recv().await {
        match update {
            Update::Sol(lamports) => sol_lamports = lamports,
            Update::Token { account, amount } => {
                if let Some(h) = holdings.get_mut(&account) {
                    h.amount = amount;
                }
            }
        }
        render(&wallet, sol_lamports, &holdings);
    }

    Ok(())
}

fn render(wallet: &str, sol_lamports: u64, holdings: &HashMap<String, Holding>) {
    print!("\x1B[2J\x1B[H");
    println!("wallet: {wallet}\n");
    println!("  SOL  {:>14.9}", sol_lamports as f64 / 1_000_000_000.0);
    for h in holdings.values() {
        let amt = h.amount as f64 / 10f64.powi(h.decimals as i32);
        let short = format!("{}...{}", &h.mint[..4], &h.mint[h.mint.len() - 4..]);
        println!("  {:>4} {:>14.6}", short, amt);
    }
    use std::io::Write;
    std::io::stdout().flush().ok();
}
