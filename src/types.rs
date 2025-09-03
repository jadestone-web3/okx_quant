use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 交易对符号
pub type Symbol = String;

/// OKX WebSocket Ticker 数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerData {
    #[serde(rename = "instId")]
    pub inst_id: String, // 交易对ID
    pub last: String, // 最新价格
    #[serde(rename = "lastSz")]
    pub last_sz: String, // 最新交易数量
    #[serde(rename = "askPx")]
    pub ask_px: String, // 卖一价
    #[serde(rename = "askSz")]
    pub ask_sz: String, // 卖一数量
    #[serde(rename = "bidPx")]
    pub bid_px: String, // 买一价
    #[serde(rename = "bidSz")]
    pub bid_sz: String, // 买一数量
    pub open24h: String, // 24小时开盘价
    pub high24h: String, // 24小时最高价
    pub low24h: String, // 24小时最低价
    #[serde(rename = "volCcy24h")]
    pub vol_ccy24h: String, // 24小时成交量(计价货币)
    pub vol24h: String, // 24小时成交量(基础货币)
    pub ts: String,   // 时间戳
}

/// OKX REST API K线数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleData {
    pub timestamp: DateTime<Utc>, // 时间戳
    pub symbol: String,           // 交易对
    pub open: f64,                // 开盘价
    pub high: f64,                // 最高价
    pub low: f64,                 // 最低价
    pub close: f64,               // 收盘价
    pub volume: f64,              // 成交量
}

/// 交易信号类型
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SignalType {
    Buy,  // 买入信号
    Sell, // 卖出信号
    Hold, // 持有信号
}

/// 交易信号
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,           // 交易对
    pub signal_type: SignalType,  // 信号类型
    pub price: f64,               // 触发价格
    pub timestamp: DateTime<Utc>, // 生成时间
    pub strategy: String,         // 策略名称
    pub reason: String,           // 信号原因
    pub confidence: f64,          // 信号置信度 (0.0-1.0)
}

/// 交易记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: Option<i64>,          // 交易ID
    pub symbol: String,           // 交易对
    pub side: String,             // 买卖方向 ("buy" or "sell")
    pub price: f64,               // 成交价格
    pub quantity: f64,            // 成交数量
    pub timestamp: DateTime<Utc>, // 成交时间
    pub strategy: String,         // 执行策略
    pub pnl: Option<f64>,         // 盈亏 (仅对已平仓交易)
}

/// 持仓信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,           // 交易对
    pub quantity: f64,            // 持仓数量
    pub avg_price: f64,           // 平均成本价
    pub current_price: f64,       // 当前价格
    pub unrealized_pnl: f64,      // 未实现盈亏
    pub timestamp: DateTime<Utc>, // 更新时间
}

/// 回测报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestReport {
    pub initial_balance: f64,      // 初始资金
    pub final_balance: f64,        // 最终资金
    pub total_return: f64,         // 总收益
    pub return_rate: f64,          // 收益率
    pub max_drawdown: f64,         // 最大回撤
    pub total_trades: usize,       // 总交易次数
    pub win_rate: f64,             // 胜率
    pub avg_return: f64,           // 平均每笔收益
    pub sharpe_ratio: f64,         // 夏普比率
    pub start_time: DateTime<Utc>, // 回测开始时间
    pub end_time: DateTime<Utc>,   // 回测结束时间
}

/// 海龟策略参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurtleParams {
    pub entry_period: usize, // 入场周期 (默认20)
    pub exit_period: usize,  // 离场周期 (默认10)
    pub atr_period: usize,   // ATR周期 (默认20)
    pub risk_per_trade: f64, // 每笔交易风险 (默认0.02, 即2%)
    pub max_units: usize,    // 最大仓位单位 (默认4)
}

impl Default for TurtleParams {
    fn default() -> Self {
        Self {
            entry_period: 20,
            exit_period: 10,
            atr_period: 20,
            risk_per_trade: 0.02,
            max_units: 4,
        }
    }
}

/// WebSocket 消息结构
#[derive(Debug, Deserialize)]
pub struct WsMessage {
    pub arg: WsArg,            // 频道参数
    pub data: Vec<TickerData>, // 数据数组
}

#[derive(Debug, Deserialize)]
pub struct WsArg {
    pub channel: String, // 频道名称
    #[serde(rename = "instId")]
    pub inst_id: String, // 交易对ID
}
