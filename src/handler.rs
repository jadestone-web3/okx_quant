use serde_json::Value;

use crate::strategy::process_ticker;

pub async fn handle_message(msg: String) {
    let parsed: serde_json::Result<Value> = serde_json::from_str(&msg);

    match parsed {
        Ok(v) => {
            if let Some(arg) = v.get("arg") {
                let channel = arg.get("channel").and_then(|c| c.as_str()).unwrap_or("");

                match channel {
                    "tickers" => {
                        println!("📈 Ticker: {}", v);
                        process_ticker(v).await;
                    }
                    "candle1m" => {
                        println!("🕯️ Candle: {}", v);
                    }
                    _ => {
                        println!("🔔 其他消息: {}", v);
                    }
                }
            } else if v.get("event").is_some() {
                println!("⚡ 系统消息: {}", v);
            }
        }
        Err(_) => {
            println!("⚠️ 无法解析: {}", msg);
        }
    }
}
