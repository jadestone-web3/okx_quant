use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::{
    task,
    time::{Duration, sleep},
};
use tokio_tungstenite::connect_async;

#[tokio::main]
async fn main() {
    // 共享内存，用于存价格序列
    let prices: Arc<Mutex<Vec<f64>>> = Arc::new(Mutex::new(Vec::new()));

    // 克隆引用，传给 websocket 线程
    let prices_ws = Arc::clone(&prices);

    // 启动 WebSocket 订阅任务
    task::spawn(async move {
        let url = "wss://ws.okx.com:8443/ws/v5/public";
        let (ws_stream, _) = connect_async(url).await.expect("连接失败");
        let (mut write, mut read) = ws_stream.split();

        // 订阅 SOL-USDT 实时 ticker
        let sub_msg = json!({
            "op": "subscribe",
            "args": [
                { "channel": "tickers", "instId": "SOL-USDT" }
            ]
        });
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                sub_msg.to_string(),
            ))
            .await
            .unwrap();

        // 读取数据
        while let Some(msg) = read.next().await {
            if let Ok(txt) = msg {
                if let tokio_tungstenite::tungstenite::Message::Text(txt) = txt {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if let Some(arr) = val["data"].as_array() {
                            if let Some(last) = arr[0]["last"].as_str() {
                                if let Ok(price) = last.parse::<f64>() {
                                    let mut p = prices_ws.lock().unwrap();
                                    p.push(price);
                                    if p.len() > 100 {
                                        p.remove(0); // 保持长度，避免无限增长
                                    }
                                    println!("新价格: {}", price);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // 策略线程：计算 MA5 / MA20
    loop {
        {
            let p = prices.lock().unwrap();
            if p.len() >= 20 {
                let ma5: f64 = p[p.len() - 5..].iter().sum::<f64>() / 5.0;
                let ma20: f64 = p[p.len() - 20..].iter().sum::<f64>() / 20.0;

                if ma5 > ma20 {
                    println!("📈 买入信号 (MA5 {:.2} > MA20 {:.2})", ma5, ma20);
                } else if ma5 < ma20 {
                    println!("📉 卖出信号 (MA5 {:.2} < MA20 {:.2})", ma5, ma20);
                }
            }
        }
        sleep(Duration::from_secs(2)).await; // 每 2 秒检查一次
    }
}
