use anyhow::Result;
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;

mod db;
mod handler;
mod strategy;
mod strategy_manager;
mod types;

use db::Database;
use handler::DataHandler;
use strategy_manager::StrategyManager;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    env_logger::init();
    info!("启动量化交易系统");

    // 初始化数据库
    let db = Arc::new(Database::new("trading.db").await?);
    info!("数据库初始化完成");

    // 初始化数据处理器
    let data_handler = Arc::new(DataHandler::new(db.clone()));

    // 初始化策略管理器
    let strategy_manager = Arc::new(Mutex::new(StrategyManager::new(db.clone())));

    // 启动数据收集任务
    let data_task = {
        let handler = data_handler.clone();
        tokio::spawn(async move {
            if let Err(e) = handler.start_data_collection().await {
                warn!("数据收集出错: {}", e);
            }
        })
    };

    // 启动实时交易策略
    let trading_task = {
        let manager = strategy_manager.clone();
        let handler = data_handler.clone();
        tokio::spawn(async move {
            if let Err(e) = run_real_time_trading(manager, handler).await {
                warn!("实时交易出错: {}", e);
            }
        })
    };
    loop {
        println!("\n请选择功能:");
        println!("1. 开始数据收集");
        println!("2. 运行回测");
        println!("3. 实时交易");
        println!("4. 查看交易历史");
        println!("5. 退出");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => {
                info!("数据收集已在后台运行");
            }
            "2" => {
                info!("开始回测分析...");
                run_backtest(strategy_manager.clone()).await?;
            }
            "3" => {
                info!("实时交易已在后台运行");
            }
            "4" => {
                show_trading_history(db.clone()).await?;
            }
            "5" => {
                info!("退出程序");
                break;
            }
            _ => println!("无效选择，请重新输入"),
        }
    }
    // 等待任务完成
    data_task.abort();
    trading_task.abort();

    Ok(())
}
/// 运行回测分析
async fn run_backtest(strategy_manager: Arc<Mutex<StrategyManager>>) -> Result<()> {
    let mut manager = strategy_manager.lock().await;
    // 设置回测参数
    let start_time = chrono::Utc::now() - chrono::Duration::days(30); // 最近30天
    let end_time = chrono::Utc::now();
    let initial_balance = 10000.0; // 初始资金10000 USDT
    info!("执行回测: {} 到 {}", start_time, end_time);
    // 执行回测
    let report = manager
        .run_backtest(start_time, end_time, initial_balance)
        .await?;
    // 打印回测报告
    println!("\n===== 回测报告 =====");
    println!("初始资金: ${:.2}", report.initial_balance);
    println!("最终资金: ${:.2}", report.final_balance);
    println!("总收益: ${:.2}", report.total_return);
    println!("收益率: {:.2}%", report.return_rate * 100.0);
    println!("最大回撤: {:.2}%", report.max_drawdown * 100.0);
    println!("交易次数: {}", report.total_trades);
    println!("胜率: {:.2}%", report.win_rate * 100.0);
    println!("平均收益: ${:.2}", report.avg_return);
    println!("夏普比率: {:.2}", report.sharpe_ratio);

    Ok(())
}

/// 运行实时交易
async fn run_real_time_trading(
    strategy_manager: Arc<Mutex<StrategyManager>>,
    data_handler: Arc<DataHandler>,
) -> Result<()> {
    info!("开始实时交易监控");
    // 订阅实时价格更新
    let mut receiver = data_handler.subscribe_price_updates().await;
    while let Ok(ticker_data) = receiver.recv().await {
        let mut manager = strategy_manager.lock().await;
        // 处理实时数据，生成交易信号
        if let Some(signal) = manager.process_real_time_data(&ticker_data).await? {
            info!("生成交易信号: {:?}", signal);

            // 这里可以添加实际的交易执行逻辑
            // execute_trade(&signal).await?;
        }
    }

    Ok(())
}
/// 显示交易历史
async fn show_trading_history(db: Arc<Database>) -> Result<()> {
    let trades = db.get_recent_trades(50).await?;

    println!("\n===== 最近交易历史 =====");
    for trade in trades {
        println!(
            "{} | {} | {} | 价格: ${:.4} | 数量: {:.4} | PnL: ${:.2}",
            trade.timestamp.format("%Y-%m-%d %H:%M:%S"),
            trade.symbol,
            trade.side,
            trade.price,
            trade.quantity,
            trade.pnl.unwrap_or(0.0)
        );
    }

    Ok(())
}
