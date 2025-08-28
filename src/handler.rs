use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db;
use crate::strategy::Candle;
use crate::strategy_manager::StrategyManager;

pub async fn handle_message(txt: String, manager: Arc<Mutex<StrategyManager>>) {
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        eprintln!("⚠️ JSON解析失败: {}", txt);
        return;
    };

    // 订阅确认 / 系统事件
    if v.get("event").is_some() {
        println!("ℹ️ 系统消息: {}", v);
        return;
    }

    // 公共数据
    // OKX WS 数据结构一般为 { arg: {channel, instId}, data: [...] }
    let Some(arg) = v.get("arg") else {
        return;
    };
    let channel = arg.get("channel").and_then(|s| s.as_str()).unwrap_or("");
    let inst_id = arg.get("instId").and_then(|s| s.as_str()).unwrap_or("");

    match channel {
        "tickers" => {
            if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
                if let Some(first) = arr.first() {
                    let inst_id = first.get("instId").and_then(|v| v.as_str()).unwrap_or("");
                    let last_px = first.get("last").and_then(|v| v.as_str()).unwrap_or("0");
                    let price = last_px.parse::<f64>().unwrap_or(0.0);
                    print!("{} {}", inst_id, price);
                }
            }
        }
        "candle1m" => {
            if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
                for row in arr {
                    // 优先适配数组格式
                    if let Some(a) = row.as_array() {
                        // 保险起见做下标检查
                        if a.len() >= 5 {
                            let ts_ms = a[0]
                                .as_str()
                                .and_then(|s| s.parse::<i64>().ok())
                                .unwrap_or(0);
                            let o = a[1]
                                .as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            let h = a[2]
                                .as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            let l = a[3]
                                .as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            let c = a[4]
                                .as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);

                            // 入库
                            if let Err(e) = db::save_candle(inst_id, ts_ms, o, h, l, c) {
                                eprintln!("保存 candle 失败: {:?}", e);
                            }

                            // 分发给策略
                            let candle = Candle {
                                ts: ts_ms,
                                open: o,
                                high: h,
                                low: l,
                                close: c,
                            };
                            manager.lock().await.on_new_candle(inst_id, candle);
                        }
                    } else {
                        // 兼容对象格式（极少数网关/代理可能转换）
                        let ts_ms = row.get("ts").and_then(|x| x.as_i64()).unwrap_or(0);
                        let o = row
                            .get("open")
                            .and_then(|x| x.as_str())
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        let h = row
                            .get("high")
                            .and_then(|x| x.as_str())
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        let l = row
                            .get("low")
                            .and_then(|x| x.as_str())
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        let c = row
                            .get("close")
                            .and_then(|x| x.as_str())
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);

                        if let Err(e) = db::save_candle(inst_id, ts_ms, o, h, l, c) {
                            eprintln!("保存 candle 失败: {:?}", e);
                        }
                        let candle = Candle {
                            ts: ts_ms,
                            open: o,
                            high: h,
                            low: l,
                            close: c,
                        };
                        manager.lock().await.on_new_candle(inst_id, candle);
                    }
                }
            }
        }
        _ => {}
    }
}
