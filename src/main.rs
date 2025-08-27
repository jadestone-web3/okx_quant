mod handler;
mod strategy;
mod ws;

#[tokio::main]
async fn main() {
    println!("🚀 OKX WS Client 启动中...");

    // 启动 WebSocket 客户端
    if let Err(e) = ws::start().await {
        eprintln!("❌ WS 运行失败: {:?}", e);
    }
}
