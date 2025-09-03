use anyhow::Result;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

use crate::db::Database;
use crate::types::{CandleData, TickerData, WsMessage};

/// 数据处理器
pub struct DataHandler {
    db: Arc<Database>,                           // 数据库实例
    price_sender: broadcast::Sender<TickerData>, // 价格数据广播
}

impl DataHandler {
    /// 创建新的数据处理器
    pub fn new(db: Arc<Database>) -> Self {
        let (price_sender, _) = broadcast::channel(1000);

        Self { db, price_sender }
    }

    /// 开始数据收集
    pub async fn start_data_collection(&self) -> Result<()> {
        info!("开始数据收集");

        // 启动WebSocket数据收集
        let ws_task = {
            let db = self.db.clone();
            let sender = self.price_sender.clone();
            tokio::spawn(async move {
                if let Err(e) = collect_websocket_data(db, sender).await {
                    error!("WebSocket数据收集错误: {}", e);
                }
            })
        };

        // 启动REST API历史数据收集
        let rest_task = {
            let db = self.db.clone();
            tokio::spawn(async move {
                if let Err(e) = collect_historical_data(db).await {
                    error!("历史数据收集错误: {}", e);
                }
            })
        };

        // 等待任务完成
        tokio::try_join!(ws_task, rest_task)?;

        Ok(())
    }

    /// 订阅价格更新
    pub async fn subscribe_price_updates(&self) -> broadcast::Receiver<TickerData> {
        self.price_sender.subscribe()
    }
}

/// 通过WebSocket收集实时数据
async fn collect_websocket_data(
    db: Arc<Database>,
    price_sender: broadcast::Sender<TickerData>,
) -> Result<()> {
    let ws_url = "wss://ws.okx.com:8443/ws/v5/public";
    info!("连接WebSocket: {}", ws_url);

    let url = Url::parse(ws_url)?;
    let (ws_stream, _) = connect_async(url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // 订阅SOL-USDT ticker数据
    let subscribe_msg = json!({
        "op": "subscribe",
        "args": [{
            "channel": "tickers",
            "instId": "SOL-USDT"
        }]
    });

    ws_sender
        .send(Message::Text(subscribe_msg.to_string()))
        .await?;

    info!("已订阅SOL-USDT ticker数据");

    // 处理接收到的消息
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) = process_ws_message(&text, &db, &price_sender).await {
                    warn!("处理WebSocket消息失败: {}", e);
                }
            }
            Ok(Message::Ping(ping)) => {
                // 响应ping消息
                if let Err(e) = ws_sender.send(Message::Pong(ping)).await {
                    error!("发送pong失败: {}", e);
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket连接关闭");
                break;
            }
            Err(e) => {
                error!("WebSocket错误: {}", e);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// 处理WebSocket消息
async fn process_ws_message(
    text: &str,
    db: &Database,
    price_sender: &broadcast::Sender<TickerData>,
) -> Result<()> {
    // 解析消息
    let value: Value = serde_json::from_str(text)?;

    // 检查是否包含数据
    if let Some(data_array) = value.get("data") {
        if let Ok(ws_msg) = serde_json::from_value::<WsMessage>(value) {
            for ticker in ws_msg.data {
                // 保存到数据库
                if let Err(e) = db.save_ticker(&ticker).await {
                    warn!("保存ticker数据失败: {}", e);
                }

                // 广播价格更新
                if price_sender.send(ticker).is_err() {
                    warn!("广播价格更新失败，可能没有订阅者");
                }
            }
        }
    }

    Ok(())
}

/// 收集历史K线数据
async fn collect_historical_data(db: Arc<Database>) -> Result<()> {
    let client = reqwest::Client::new();
    let symbol = "SOL-USDT";

    info!("开始收集{}的历史K线数据", symbol);

    // 分页回填：目标回填数量与时间范围可按需调整
    let target_backfill_count: usize = 5000;
    let page_limit: usize = 300; // OKX 单页上限通常为 300

    let mut total_collected: usize = 0;
    let mut before: Option<i64> = None; // 毫秒时间戳，OKX 使用 before 游标

    loop {
        let page = fetch_candles_page(&client, symbol, page_limit, before).await?;
        if page.is_empty() {
            info!("历史回填结束，未返回更多数据，累计{}条", total_collected);
            break;
        }

        // OKX 返回通常是按时间倒序（新->旧），为了写库前可直接入库
        db.save_candles(&page).await?;
        total_collected += page.len();

        // 更新 before 为本页中最早的一根的时间戳（更旧）
        let oldest_ts = page.iter().map(|c| c.timestamp.timestamp_millis()).min().unwrap_or(0);
        before = Some(oldest_ts);

        info!("历史回填进行中：本页{}条，累计{}条", page.len(), total_collected);

        if total_collected >= target_backfill_count {
            info!("达到目标回填数量{}条，停止回填", target_backfill_count);
            break;
        }

        // 简单节流，避免过快请求
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    info!("历史数据回填完成，共{}条记录", total_collected);

    // 改为每分钟增量更新，带冗余覆盖（取最近300条）
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    loop {
        interval.tick().await;

        info!("定时更新历史数据（每分钟）");
        if let Err(e) = update_recent_candles(&db, &client, symbol).await {
            warn!("定时更新失败: {}", e);
        }
    }
}

/// 拉取一页 OKX 1m K线
async fn fetch_candles_page(
    client: &reqwest::Client,
    symbol: &str,
    limit: usize,
    before: Option<i64>,
) -> Result<Vec<CandleData>> {
    // OKX 文档：/api/v5/market/candles?instId=...&bar=1m&limit=...&before=...
    let mut url = format!(
        "https://www.okx.com/api/v5/market/candles?instId={}&bar=1m&limit={}",
        symbol, limit
    );
    if let Some(ts) = before {
        url.push_str(&format!("&before={}", ts));
    }

    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("API请求失败: {}", response.status()));
    }

    let json_response: Value = response.json().await?;
    if json_response["code"] != "0" {
        return Err(anyhow::anyhow!("API返回错误: {}", json_response["msg"]));
    }

    let data = json_response["data"].as_array().ok_or_else(|| anyhow::anyhow!("无效的API响应格式"))?;
    let mut candles = Vec::new();

    for candle_data in data {
        if let Some(candle_array) = candle_data.as_array() {
            if candle_array.len() >= 6 {
                let timestamp_ms: i64 = candle_array[0].as_str().unwrap_or("0").parse().unwrap_or(0);
                let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

                let candle = CandleData {
                    timestamp,
                    symbol: symbol.to_string(),
                    open: candle_array[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    high: candle_array[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    low: candle_array[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    close: candle_array[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    volume: candle_array[5].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                };

                candles.push(candle);
            }
        }
    }

    Ok(candles)
}

/// 更新最近的K线数据（带冗余覆盖）
async fn update_recent_candles(
    db: &Database,
    client: &reqwest::Client,
    symbol: &str,
) -> Result<()> {
    // 拉取最近 300 条 1m K 线，依靠 UNIQUE(timestamp, symbol) 实现幂等覆盖
    let url = format!(
        "https://www.okx.com/api/v5/market/candles?instId={}&bar=1m&limit=300",
        symbol
    );

    let response = client.get(&url).send().await?;
    let json_response: Value = response.json().await?;

    if json_response["code"] != "0" {
        return Err(anyhow::anyhow!("API返回错误: {}", json_response["msg"]));
    }

    let data = json_response["data"].as_array().ok_or_else(|| anyhow::anyhow!("无效的API响应格式"))?;

    let mut candles = Vec::new();

    for candle_data in data {
        if let Some(candle_array) = candle_data.as_array() {
            if candle_array.len() >= 6 {
                let timestamp_ms: i64 = candle_array[0].as_str().unwrap_or("0").parse().unwrap_or(0);
                let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

                let candle = CandleData {
                    timestamp,
                    symbol: symbol.to_string(),
                    open: candle_array[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    high: candle_array[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    low: candle_array[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    close: candle_array[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    volume: candle_array[5].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                };

                candles.push(candle);
            }
        }
    }

    if !candles.is_empty() {
        db.save_candles(&candles).await?;
        info!("更新了{}条K线数据", candles.len());
    }

    Ok(())
}
