mod db;
mod handler;
mod strategy;
mod strategy_manager;
mod ws;

use std::sync::Arc;
use tokio::sync::Mutex;

use strategy::{MaCrossStrategy, McStrategy};
use strategy_manager::StrategyManager;

#[tokio::main]
async fn main() {
    println!("🚀 OKX WS Client 启动中...");

    // 初始化策略管理器
    let mut manager = StrategyManager::new(200); // 每个品种保留最近200根K
    // 注册策略：你的MC策略
    manager.add_strategy(Box::new(McStrategy::new(
        0.0032, /* KC */
        4,      /* KS */
        0.0021, /* pls */
        0.0039, /* ply */
        12,     /* TT */
    )));

    manager.add_strategy(Box::new(MaCrossStrategy::new(5, 20)));

    let manager = Arc::new(Mutex::new(manager));

    // 启动 WebSocket 客户端
    if let Err(e) = ws::start(Arc::clone(&manager)).await {
        eprintln!("❌ WS 运行失败: {:?}", e);
    }
}
