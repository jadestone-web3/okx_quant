use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::Database;
use crate::strategy::{IndicatorSummary, TurtleStrategy};
use crate::types::{
    BacktestReport, CandleData, Position, SignalType, TickerData, Trade, TradingSignal,
    TurtleParams,
};

/// 策略管理器
pub struct StrategyManager {
    db: Arc<Database>,                           // 数据库实例
    strategies: HashMap<String, TurtleStrategy>, // 策略实例映射
    positions: HashMap<String, Position>,        // 当前持仓
    balance: f64,                                // 账户余额
}

impl StrategyManager {
    /// 创建新的策略管理器
    pub fn new(db: Arc<Database>) -> Self {
        let mut strategies = HashMap::new();

        // 初始化SOL-USDT的海龟策略
        let turtle_strategy = TurtleStrategy::new("SOL-USDT".to_string(), None);
        strategies.insert("SOL-USDT".to_string(), turtle_strategy);

        Self {
            db,
            strategies,
            positions: HashMap::new(),
            balance: 10000.0, // 默认10000 USDT
        }
    }

    /// 处理实时数据并生成交易信号
    pub async fn process_real_time_data(
        &mut self,
        ticker: &TickerData,
    ) -> Result<Option<TradingSignal>> {
        let symbol = &ticker.inst_id;

        // 获取最近的K线数据用于分析
        let candles = self.db.get_latest_candles(symbol, 100).await?;

        if candles.is_empty() {
            warn!("没有找到{}的K线数据", symbol);
            return Ok(None);
        }

        // 获取对应的策略
        if let Some(strategy) = self.strategies.get(symbol) {
            let signals = strategy.analyze(&candles)?;

            for signal in signals {
                // 保存信号到数据库
                self.db.save_signal(&signal).await?;

                // 执行交易逻辑
                if let Some(trade) = self.execute_signal(&signal).await? {
                    info!("执行交易: {:?}", trade);
                    return Ok(Some(signal));
                }
            }
        }

        Ok(None)
    }

    /// 执行交易信号
    async fn execute_signal(&mut self, signal: &TradingSignal) -> Result<Option<Trade>> {
        let symbol = &signal.symbol;

        // 获取当前持仓
        let current_position = self.positions.get(symbol).cloned();

        match signal.signal_type {
            SignalType::Buy => {
                if current_position.is_none() || current_position.as_ref().unwrap().quantity <= 0.0
                {
                    // 开多仓或平空仓
                    return self.open_long_position(signal).await;
                }
            }
            SignalType::Sell => {
                if current_position.is_none() || current_position.as_ref().unwrap().quantity >= 0.0
                {
                    // 开空仓或平多仓
                    return self.open_short_position(signal).await;
                }
            }
            SignalType::Hold => {
                // 持有信号，暂不处理
                return Ok(None);
            }
        }

        Ok(None)
    }

    /// 开多仓
    async fn open_long_position(&mut self, signal: &TradingSignal) -> Result<Option<Trade>> {
        let symbol = &signal.symbol;

        // 获取策略和ATR计算仓位大小
        if let Some(strategy) = self.strategies.get(symbol) {
            let candles = self.db.get_latest_candles(symbol, 50).await?;

            if !candles.is_empty() {
                let indicators = strategy.calculate_indicators(&candles)?;

                if let Some(atr) = indicators.atr {
                    // 计算仓位大小
                    let position_size =
                        strategy.calculate_position_size(self.balance, signal.price, atr);

                    if position_size > 0.0 && position_size * signal.price <= self.balance * 0.95 {
                        // 创建交易记录
                        let trade = Trade {
                            id: None,
                            symbol: symbol.clone(),
                            side: "buy".to_string(),
                            price: signal.price,
                            quantity: position_size,
                            timestamp: signal.timestamp,
                            strategy: signal.strategy.clone(),
                            pnl: None,
                        };

                        // 保存交易到数据库
                        let trade_id = self.db.save_trade(&trade).await?;

                        // 更新持仓
                        let position = Position {
                            symbol: symbol.clone(),
                            quantity: position_size,
                            avg_price: signal.price,
                            current_price: signal.price,
                            unrealized_pnl: 0.0,
                            timestamp: signal.timestamp,
                        };

                        self.positions.insert(symbol.clone(), position);

                        // 更新账户余额
                        self.balance -= position_size * signal.price;

                        info!(
                            "开多仓成功: {} @ {:.4}, 数量: {:.4}",
                            symbol, signal.price, position_size
                        );

                        let mut executed_trade = trade;
                        executed_trade.id = Some(trade_id);
                        return Ok(Some(executed_trade));
                    }
                }
            }
        }

        Ok(None)
    }

    /// 开空仓
    async fn open_short_position(&mut self, signal: &TradingSignal) -> Result<Option<Trade>> {
        let symbol = &signal.symbol;

        // 获取当前持仓，如果是多仓则平仓
        if let Some(position) = self.positions.get(symbol) {
            if position.quantity > 0.0 {
                return self.close_long_position(signal).await;
            }
        }

        // 这里可以添加开空仓逻辑，现货交易通常不支持做空
        // 暂时只实现平多仓逻辑
        Ok(None)
    }

    /// 平多仓
    async fn close_long_position(&mut self, signal: &TradingSignal) -> Result<Option<Trade>> {
        let symbol = &signal.symbol;

        if let Some(position) = self.positions.get(symbol).cloned() {
            if position.quantity > 0.0 {
                // 计算盈亏
                let pnl = (signal.price - position.avg_price) * position.quantity;

                // 创建平仓交易记录
                let trade = Trade {
                    id: None,
                    symbol: symbol.clone(),
                    side: "sell".to_string(),
                    price: signal.price,
                    quantity: position.quantity,
                    timestamp: signal.timestamp,
                    strategy: signal.strategy.clone(),
                    pnl: Some(pnl),
                };

                // 保存交易到数据库
                let trade_id = self.db.save_trade(&trade).await?;

                // 更新账户余额
                self.balance += position.quantity * signal.price;

                // 清除持仓
                self.positions.remove(symbol);

                info!(
                    "平多仓成功: {} @ {:.4}, 盈亏: {:.2}",
                    symbol, signal.price, pnl
                );

                let mut executed_trade = trade;
                executed_trade.id = Some(trade_id);
                return Ok(Some(executed_trade));
            }
        }

        Ok(None)
    }

    /// 运行回测
    pub async fn run_backtest(
        &mut self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        initial_balance: f64,
    ) -> Result<BacktestReport> {
        info!("开始回测: {} 到 {}", start_time, end_time);

        // 重置回测环境
        self.balance = initial_balance;
        self.positions.clear();

        let mut trades = Vec::new();
        let mut equity_curve = Vec::new();

        // 获取回测期间的K线数据
        let symbol = "SOL-USDT";
        let candles = self
            .db
            .get_candles(symbol, start_time, end_time, None)
            .await?;

        if candles.len() < 50 {
            return Err(anyhow::anyhow!("回测数据不足，需要至少50根K线"));
        }

        info!("回测数据: {} 根K线", candles.len());

        // 获取策略
        let strategy = self
            .strategies
            .get(symbol)
            .ok_or_else(|| anyhow::anyhow!("未找到{}的策略", symbol))?;

        // 逐根K线进行回测
        for i in 50..candles.len() {
            let current_candles = &candles[0..=i];
            let current_candle = &candles[i];

            // 分析当前数据
            let signals = strategy.analyze(current_candles)?;

            // 处理生成的信号
            for signal in signals {
                if let Some(trade) = self.simulate_trade(&signal, current_candle).await? {
                    trades.push(trade);
                }
            }

            // 更新持仓的当前价格
            self.update_positions_price(symbol, current_candle.close);

            // 记录权益曲线
            let total_equity = self.calculate_total_equity();
            equity_curve.push((current_candle.timestamp, total_equity));
        }

        // 生成回测报告
        let report = self.generate_backtest_report(
            initial_balance,
            &trades,
            &equity_curve,
            start_time,
            end_time,
        )?;

        info!("回测完成，共执行{}笔交易", trades.len());

        Ok(report)
    }

    /// 模拟交易执行
    async fn simulate_trade(
        &mut self,
        signal: &TradingSignal,
        candle: &CandleData,
    ) -> Result<Option<Trade>> {
        let symbol = &signal.symbol;
        let current_position = self.positions.get(symbol).cloned();

        match signal.signal_type {
            SignalType::Buy => {
                if current_position.is_none() || current_position.as_ref().unwrap().quantity <= 0.0
                {
                    return self.simulate_long_entry(signal, candle).await;
                }
            }
            SignalType::Sell => {
                if let Some(position) = current_position {
                    if position.quantity > 0.0 {
                        return self.simulate_long_exit(signal, candle).await;
                    }
                }
            }
            SignalType::Hold => {}
        }

        Ok(None)
    }

    /// 模拟开多仓
    async fn simulate_long_entry(
        &mut self,
        signal: &TradingSignal,
        candle: &CandleData,
    ) -> Result<Option<Trade>> {
        let symbol = &signal.symbol;

        if let Some(strategy) = self.strategies.get(symbol) {
            let candles = vec![candle.clone()]; // 简化处理，实际应该传入历史数据
            let indicators = strategy.calculate_indicators(&candles)?;

            if let Some(atr) = indicators.atr {
                let position_size =
                    strategy.calculate_position_size(self.balance, signal.price, atr);
                let trade_value = position_size * signal.price;

                if position_size > 0.0 && trade_value <= self.balance * 0.95 {
                    let trade = Trade {
                        id: None,
                        symbol: symbol.clone(),
                        side: "buy".to_string(),
                        price: signal.price,
                        quantity: position_size,
                        timestamp: signal.timestamp,
                        strategy: signal.strategy.clone(),
                        pnl: None,
                    };

                    // 更新持仓
                    let position = Position {
                        symbol: symbol.clone(),
                        quantity: position_size,
                        avg_price: signal.price,
                        current_price: signal.price,
                        unrealized_pnl: 0.0,
                        timestamp: signal.timestamp,
                    };

                    self.positions.insert(symbol.clone(), position);
                    self.balance -= trade_value;

                    return Ok(Some(trade));
                }
            }
        }

        Ok(None)
    }

    /// 模拟平多仓
    async fn simulate_long_exit(
        &mut self,
        signal: &TradingSignal,
        candle: &CandleData,
    ) -> Result<Option<Trade>> {
        let symbol = &signal.symbol;

        if let Some(position) = self.positions.get(symbol).cloned() {
            if position.quantity > 0.0 {
                let pnl = (signal.price - position.avg_price) * position.quantity;

                let trade = Trade {
                    id: None,
                    symbol: symbol.clone(),
                    side: "sell".to_string(),
                    price: signal.price,
                    quantity: position.quantity,
                    timestamp: signal.timestamp,
                    strategy: signal.strategy.clone(),
                    pnl: Some(pnl),
                };

                self.balance += position.quantity * signal.price;
                self.positions.remove(symbol);

                return Ok(Some(trade));
            }
        }

        Ok(None)
    }

    /// 更新持仓价格
    fn update_positions_price(&mut self, symbol: &str, current_price: f64) {
        if let Some(position) = self.positions.get_mut(symbol) {
            position.current_price = current_price;
            position.unrealized_pnl = (current_price - position.avg_price) * position.quantity;
        }
    }

    /// 计算总权益
    fn calculate_total_equity(&self) -> f64 {
        let mut total_equity = self.balance;

        for position in self.positions.values() {
            total_equity += position.quantity * position.current_price;
        }

        total_equity
    }

    /// 生成回测报告
    fn generate_backtest_report(
        &self,
        initial_balance: f64,
        trades: &[Trade],
        equity_curve: &[(DateTime<Utc>, f64)],
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<BacktestReport> {
        let final_balance = self.calculate_total_equity();
        let total_return = final_balance - initial_balance;
        let return_rate = total_return / initial_balance;

        // 计算最大回撤
        let max_drawdown = self.calculate_max_drawdown(equity_curve);

        // 计算交易统计
        let total_trades = trades.len();
        let profitable_trades = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) > 0.0).count();

        let win_rate = if total_trades > 0 {
            profitable_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        let avg_return = if total_trades > 0 {
            trades.iter().filter_map(|t| t.pnl).sum::<f64>() / total_trades as f64
        } else {
            0.0
        };

        // 计算夏普比率 (简化版本)
        let sharpe_ratio = self.calculate_sharpe_ratio(equity_curve)?;

        Ok(BacktestReport {
            initial_balance,
            final_balance,
            total_return,
            return_rate,
            max_drawdown,
            total_trades,
            win_rate,
            avg_return,
            sharpe_ratio,
            start_time,
            end_time,
        })
    }

    /// 计算最大回撤
    fn calculate_max_drawdown(&self, equity_curve: &[(DateTime<Utc>, f64)]) -> f64 {
        if equity_curve.len() < 2 {
            return 0.0;
        }

        let mut max_equity = equity_curve[0].1;
        let mut max_drawdown = 0.0;

        for (_, equity) in equity_curve.iter().skip(1) {
            if *equity > max_equity {
                max_equity = *equity;
            }

            let drawdown = (max_equity - equity) / max_equity;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        max_drawdown
    }

    /// 计算夏普比率
    fn calculate_sharpe_ratio(&self, equity_curve: &[(DateTime<Utc>, f64)]) -> Result<f64> {
        if equity_curve.len() < 2 {
            return Ok(0.0);
        }

        // 计算日收益率
        let mut daily_returns = Vec::new();
        for i in 1..equity_curve.len() {
            let prev_equity = equity_curve[i - 1].1;
            let curr_equity = equity_curve[i].1;
            let daily_return = (curr_equity - prev_equity) / prev_equity;
            daily_returns.push(daily_return);
        }

        if daily_returns.is_empty() {
            return Ok(0.0);
        }

        // 计算平均收益率和标准差
        let mean_return = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;

        let variance = daily_returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / daily_returns.len() as f64;

        let std_dev = variance.sqrt();

        // 夏普比率 = (平均收益率 - 无风险收益率) / 标准差
        // 假设无风险收益率为0
        if std_dev > 0.0 {
            Ok(mean_return / std_dev * (365.0_f64).sqrt()) // 年化
        } else {
            Ok(0.0)
        }
    }

    /// 添加策略
    pub fn add_strategy(&mut self, symbol: String, params: Option<TurtleParams>) -> Result<()> {
        let strategy = TurtleStrategy::new(symbol.clone(), params);
        self.strategies.insert(symbol.clone(), strategy);
        info!("添加策略: {}", symbol);
        Ok(())
    }

    /// 获取当前持仓
    pub fn get_positions(&self) -> &HashMap<String, Position> {
        &self.positions
    }

    /// 获取当前余额
    pub fn get_balance(&self) -> f64 {
        self.balance
    }

    /// 更新余额
    pub fn set_balance(&mut self, balance: f64) {
        self.balance = balance;
    }

    /// 获取策略参数
    pub fn get_strategy_params(&self, symbol: &str) -> Option<&TurtleParams> {
        self.strategies.get(symbol).map(|s| s.get_params())
    }

    /// 更新策略参数
    pub fn update_strategy_params(&mut self, symbol: &str, params: TurtleParams) -> Result<()> {
        if let Some(strategy) = self.strategies.get_mut(symbol) {
            TurtleStrategy::validate_params(&params)?;
            strategy.update_params(params);
            Ok(())
        } else {
            Err(anyhow::anyhow!("策略不存在: {}", symbol))
        }
    }
}
