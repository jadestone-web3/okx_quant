use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::handler::handle_message;
use crate::strategy_manager::StrategyManager;

const WS_URL: &str = "wss://ws.okx.com:8443/ws/v5/public";

pub async fn start(manager: Arc<Mutex<StrategyManager>>) -> anyhow::Result<()> {
    let (ws_stream, _) = connect_async(WS_URL).await?;
    println!("✅ 已连接 OKX WS: {}", WS_URL);

    let (mut write, mut read) = ws_stream.split();

    // 使用 Arc<Mutex> 管理写入，方便心跳 task 使用
    let write = Arc::new(Mutex::new(write));

    // 订阅 SOL-USDT ticker & 1m candle
    let sub_msg = r#"{
        "op":"subscribe",
        "args":[
            {"channel":"tickers","instId":"SOL-USDT"}
        ]
    }"#;

    // let sub_msg = r#"{
    //     "op":"subscribe",
    //     "args":[
    //        {"channel":"candle5m","instId":"SOL-USDT"}
    //     ]
    // }"#;

    write
        .lock()
        .await
        .send(Message::Text(sub_msg.into()))
        .await?;

    // 读取消息
    while let Some(msg) = read.next().await {
        match msg {
            Ok(tokio_tungstenite::tungstenite::Message::Text(txt)) => {
                handle_message(txt, Arc::clone(&manager)).await;
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
