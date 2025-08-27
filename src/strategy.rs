use serde_json::Value;

/// 策略入口：处理 Ticker
pub async fn process_ticker(data: Value) {
    if let Some(arr) = data.get("data").and_then(|d| d.as_array()) {
        if let Some(first) = arr.first() {
            let last_px = first.get("last").and_then(|v| v.as_str()).unwrap_or("0");
            println!("🎯 最新成交价: {}", last_px);

            // 简单策略示例：价格大于 70k 输出提示
            if last_px.parse::<f64>().unwrap_or(0.0) > 70000.0 {
                println!("🚨 价格超过 70k，触发策略信号！");
            }
        }
    }
}
