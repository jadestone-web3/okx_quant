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

    // 获取最近1000根1分钟K线
    let url = format!(
        "https://www.okx.com/api/v5/market/candles?instId={}&bar=1m&limit=1000",
        symbol
    );

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("API请求失败: {}", response.status()));
    }

    let json_response: Value = response.json().await?;

    if json_response["code"] != "0" {
        return Err(anyhow::anyhow!("API返回错误: {}", json_response["msg"]));
    }

    let data = json_response["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的API响应格式"))?;

    let mut candles = Vec::new();

    for candle_data in data {
        if let Some(candle_array) = candle_data.as_array() {
            if candle_array.len() >= 6 {
                // OKX返回的数据格式: [timestamp, open, high, low, close, volume, ...]
                let timestamp_ms: i64 =
                    candle_array[0].as_str().unwrap_or("0").parse().unwrap_or(0);

                let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

                let candle = CandleData {
                    timestamp,
                    symbol: symbol.to_string(),
                    open: candle_array[1]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    high: candle_array[2]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    low: candle_array[3]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    close: candle_array[4]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    volume: candle_array[5]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                };

                candles.push(candle);
            }
        }
    }

    // 批量保存到数据库
    db.save_candles(&candles).await?;
    info!("历史数据收集完成，共{}条记录", candles.len());

    // 每小时更新一次历史数据
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;

        info!("定时更新历史数据");
        if let Err(e) = update_recent_candles(&db, &client, symbol).await {
            warn!("定时更新失败: {}", e);
        }
    }
}

/// 更新最近的K线数据
async fn update_recent_candles(
    db: &Database,
    client: &reqwest::Client,
    symbol: &str,
) -> Result<()> {
    let url = format!(
        "https://www.okx.com/api/v5/market/candles?instId={}&bar=1m&limit=100",
        symbol
    );

    let response = client.get(&url).send().await?;
    let json_response: Value = response.json().await?;

    if json_response["code"] != "0" {
        return Err(anyhow::anyhow!("API返回错误: {}", json_response["msg"]));
    }

    let data = json_response["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("无效的API响应格式"))?;

    let mut candles = Vec::new();

    for candle_data in data {
        if let Some(candle_array) = candle_data.as_array() {
            if candle_array.len() >= 6 {
                let timestamp_ms: i64 =
                    candle_array[0].as_str().unwrap_or("0").parse().unwrap_or(0);

                let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

                let candle = CandleData {
                    timestamp,
                    symbol: symbol.to_string(),
                    open: candle_array[1]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    high: candle_array[2]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    low: candle_array[3]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    close: candle_array[4]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
                    volume: candle_array[5]
                        .as_str()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0.0),
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
