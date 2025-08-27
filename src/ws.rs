use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::handler::handle_message;

const WS_URL: &str = "wss://ws.okx.com:8443/ws/v5/public";

pub async fn start() -> anyhow::Result<()> {
    let (ws_stream, _) = connect_async(WS_URL).await?;
    println!("✅ 已连接 OKX WS: {}", WS_URL);

    let (mut write, mut read) = ws_stream.split();

    // 使用 Arc<Mutex> 管理写入，方便心跳 task 使用
    let write = Arc::new(Mutex::new(write));

    // 订阅 SOL-USDT ticker & 1m candle
    let sub_msg = r#"{
        "op":"subscribe",
        "args":[
            {"channel":"tickers","instId":"SOL-USDT"},
            {"channel":"candle1m","instId":"SOL-USDT"}
        ]
    }"#;

    write
        .lock()
        .await
        .send(Message::Text(sub_msg.into()))
        .await?;

    let write_clone = Arc::clone(&write);
    // 启动心跳定时器
    let mut interval = time::interval(Duration::from_secs(25));

    tokio::spawn(async move {
        loop {
            interval.tick().await;
            let ping = r#"{"op":"ping"}"#;
            if let Err(e) = write_clone
                .lock()
                .await
                .send(Message::Text(ping.into()))
                .await
            {
                eprintln!("⚠️ 心跳发送失败: {:?}", e);
                break;
            }
            println!("💓 已发送 ping");
        }
    });

    // 读取消息
    while let Some(msg) = read.next().await {
        match msg {
            Ok(tokio_tungstenite::tungstenite::Message::Text(txt)) => {
                handle_message(txt).await;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("❌ WS 错误: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
