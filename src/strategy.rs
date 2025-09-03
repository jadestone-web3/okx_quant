use anyhow::Result;
use chrono::{DateTime, Utc};
use log::info;

use crate::types::{CandleData, SignalType, TradingSignal, TurtleParams};

/// 海龟交易策略
pub struct TurtleStrategy {
    params: TurtleParams, // 策略参数
    symbol: String,       // 交易对
}

impl TurtleStrategy {
    /// 创建新的海龟策略实例
    pub fn new(symbol: String, params: Option<TurtleParams>) -> Self {
        Self {
            params: params.unwrap_or_default(),
            symbol,
        }
    }

    /// 分析K线数据并生成交易信号
    pub fn analyze(&self, candles: &[CandleData]) -> Result<Vec<TradingSignal>> {
        if candles.len() < self.params.entry_period.max(self.params.atr_period) {
            return Ok(vec![]); // 数据不足，无法分析
        }

        let mut signals = Vec::new();
        let latest_candle = &candles[candles.len() - 1];

        // 计算入场信号
        if let Some(entry_signal) = self.check_entry_signal(candles)? {
            signals.push(entry_signal);
        }

        // 计算离场信号
        if let Some(exit_signal) = self.check_exit_signal(candles)? {
            signals.push(exit_signal);
        }

        Ok(signals)
    }

    /// 检查入场信号
    fn check_entry_signal(&self, candles: &[CandleData]) -> Result<Option<TradingSignal>> {
        let len = candles.len();
        if len < self.params.entry_period + 1 {
            return Ok(None);
        }

        let latest_candle = &candles[len - 1];

        // 计算入场周期内的最高价和最低价
        let entry_high = self.calculate_highest_high(candles, self.params.entry_period)?;
        let entry_low = self.calculate_lowest_low(candles, self.params.entry_period)?;

        // 突破入场条件
        // 多头入场: 当前价格突破N日最高价
        if latest_candle.close > entry_high {
            let atr = self.calculate_atr(candles, self.params.atr_period)?;
            let confidence = self.calculate_confidence(candles, true)?;

            return Ok(Some(TradingSignal {
                symbol: self.symbol.clone(),
                signal_type: SignalType::Buy,
                price: latest_candle.close,
                timestamp: latest_candle.timestamp,
                strategy: "Turtle".to_string(),
                reason: format!(
                    "价格{}突破{}日最高价{:.4}，ATR={:.4}",
                    latest_candle.close, self.params.entry_period, entry_high, atr
                ),
                confidence,
            }));
        }

        // 空头入场: 当前价格突破N日最低价
        if latest_candle.close < entry_low {
            let atr = self.calculate_atr(candles, self.params.atr_period)?;
            let confidence = self.calculate_confidence(candles, false)?;

            return Ok(Some(TradingSignal {
                symbol: self.symbol.clone(),
                signal_type: SignalType::Sell,
                price: latest_candle.close,
                timestamp: latest_candle.timestamp,
                strategy: "Turtle".to_string(),
                reason: format!(
                    "价格{}跌破{}日最低价{:.4}，ATR={:.4}",
                    latest_candle.close, self.params.entry_period, entry_low, atr
                ),
                confidence,
            }));
        }

        Ok(None)
    }

    /// 检查离场信号
    fn check_exit_signal(&self, candles: &[CandleData]) -> Result<Option<TradingSignal>> {
        let len = candles.len();
        if len < self.params.exit_period + 1 {
            return Ok(None);
        }

        let latest_candle = &candles[len - 1];

        // 计算离场周期内的最高价和最低价
        let exit_high = self.calculate_highest_high(candles, self.params.exit_period)?;
        let exit_low = self.calculate_lowest_low(candles, self.params.exit_period)?;

        // 多头离场: 当前价格跌破N日最低价
        if latest_candle.close < exit_low {
            let confidence = 0.8; // 离场信号置信度较高

            return Ok(Some(TradingSignal {
                symbol: self.symbol.clone(),
                signal_type: SignalType::Sell,
                price: latest_candle.close,
                timestamp: latest_candle.timestamp,
                strategy: "Turtle_Exit".to_string(),
                reason: format!(
                    "多头离场：价格{}跌破{}日最低价{:.4}",
                    latest_candle.close, self.params.exit_period, exit_low
                ),
                confidence,
            }));
        }

        // 空头离场: 当前价格突破N日最高价
        if latest_candle.close > exit_high {
            let confidence = 0.8; // 离场信号置信度较高

            return Ok(Some(TradingSignal {
                symbol: self.symbol.clone(),
                signal_type: SignalType::Buy,
                price: latest_candle.close,
                timestamp: latest_candle.timestamp,
                strategy: "Turtle_Exit".to_string(),
                reason: format!(
                    "空头离场：价格{}突破{}日最高价{:.4}",
                    latest_candle.close, self.params.exit_period, exit_high
                ),
                confidence,
            }));
        }

        Ok(None)
    }

    /// 计算指定周期内的最高价
    fn calculate_highest_high(&self, candles: &[CandleData], period: usize) -> Result<f64> {
        let len = candles.len();
        if len < period {
            return Err(anyhow::anyhow!("数据不足以计算最高价"));
        }

        let start_idx = len - period;
        let high_prices: Vec<f64> = candles[start_idx..len - 1].iter().map(|c| c.high).collect();

        Ok(high_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)))
    }

    /// 计算指定周期内的最低价
    fn calculate_lowest_low(&self, candles: &[CandleData], period: usize) -> Result<f64> {
        let len = candles.len();
        if len < period {
            return Err(anyhow::anyhow!("数据不足以计算最低价"));
        }

        let start_idx = len - period;
        let low_prices: Vec<f64> = candles[start_idx..len - 1].iter().map(|c| c.low).collect();

        Ok(low_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b)))
    }

    /// 计算ATR (Average True Range) 平均真实波幅
    fn calculate_atr(&self, candles: &[CandleData], period: usize) -> Result<f64> {
        let len = candles.len();
        if len < period + 1 {
            return Err(anyhow::anyhow!("数据不足以计算ATR"));
        }

        let mut true_ranges = Vec::new();

        for i in 1..len {
            let current = &candles[i];
            let previous = &candles[i - 1];

            // 真实波幅 = max(高-低, |高-昨收|, |低-昨收|)
            let tr = (current.high - current.low)
                .max((current.high - previous.close).abs())
                .max((current.low - previous.close).abs());

            true_ranges.push(tr);
        }

        if true_ranges.len() < period {
            return Err(anyhow::anyhow!("真实波幅数据不足"));
        }

        // 计算最近N期的ATR
        let start_idx = true_ranges.len() - period;
        let recent_trs: Vec<f64> = true_ranges[start_idx..].to_vec();

        let atr = recent_trs.iter().sum::<f64>() / period as f64;
        Ok(atr)
    }

    /// 计算信号置信度
    fn calculate_confidence(&self, candles: &[CandleData], is_long: bool) -> Result<f64> {
        let len = candles.len();
        if len < 10 {
            return Ok(0.5); // 默认置信度
        }

        // 基于成交量和价格动量计算置信度
        let recent_candles = &candles[len - 10..];
        let avg_volume =
            recent_candles.iter().map(|c| c.volume).sum::<f64>() / recent_candles.len() as f64;

        let latest_volume = recent_candles.last().unwrap().volume;
        let volume_ratio = latest_volume / avg_volume;

        // 计算价格动量
        let price_change = if recent_candles.len() >= 2 {
            let latest_price = recent_candles.last().unwrap().close;
            let prev_price = recent_candles[recent_candles.len() - 2].close;
            (latest_price - prev_price) / prev_price
        } else {
            0.0
        };

        // 根据成交量放大和价格动量计算置信度
        let mut confidence = 0.5;

        // 成交量放大增加置信度
        if volume_ratio > 1.5 {
            confidence += 0.2;
        } else if volume_ratio > 1.2 {
            confidence += 0.1;
        }

        // 价格动量与信号方向一致增加置信度
        if (is_long && price_change > 0.0) || (!is_long && price_change < 0.0) {
            confidence += 0.1;
        }

        // 限制置信度范围在0.1到0.9之间
        Ok(confidence.max(0.1).min(0.9))
    }

    /// 计算仓位大小
    pub fn calculate_position_size(&self, balance: f64, price: f64, atr: f64) -> f64 {
        // 海龟交易法则的仓位计算
        // 风险资金 = 总资金 * 风险比例
        let risk_capital = balance * self.params.risk_per_trade;

        // 单位风险 = ATR * 价格系数 (通常为1)
        let unit_risk = atr * 1.0;

        // 仓位大小 = 风险资金 / 单位风险
        if unit_risk > 0.0 {
            risk_capital / unit_risk
        } else {
            0.0
        }
    }

    /// 获取策略参数
    pub fn get_params(&self) -> &TurtleParams {
        &self.params
    }

    /// 更新策略参数
    pub fn update_params(&mut self, params: TurtleParams) {
        self.params = params;
        info!("海龟策略参数已更新: {:?}", self.params);
    }

    /// 验证策略参数
    pub fn validate_params(params: &TurtleParams) -> Result<()> {
        if params.entry_period == 0 {
            return Err(anyhow::anyhow!("入场周期必须大于0"));
        }

        if params.exit_period == 0 {
            return Err(anyhow::anyhow!("离场周期必须大于0"));
        }

        if params.atr_period == 0 {
            return Err(anyhow::anyhow!("ATR周期必须大于0"));
        }

        if params.risk_per_trade <= 0.0 || params.risk_per_trade > 1.0 {
            return Err(anyhow::anyhow!("每笔交易风险必须在0到1之间"));
        }

        if params.max_units == 0 {
            return Err(anyhow::anyhow!("最大仓位单位必须大于0"));
        }

        Ok(())
    }

    /// 计算技术指标摘要
    pub fn calculate_indicators(&self, candles: &[CandleData]) -> Result<IndicatorSummary> {
        if candles.is_empty() {
            return Err(anyhow::anyhow!("没有K线数据"));
        }

        let latest = candles.last().unwrap();

        let entry_high = if candles.len() >= self.params.entry_period {
            Some(self.calculate_highest_high(candles, self.params.entry_period)?)
        } else {
            None
        };

        let entry_low = if candles.len() >= self.params.entry_period {
            Some(self.calculate_lowest_low(candles, self.params.entry_period)?)
        } else {
            None
        };

        let exit_high = if candles.len() >= self.params.exit_period {
            Some(self.calculate_highest_high(candles, self.params.exit_period)?)
        } else {
            None
        };

        let exit_low = if candles.len() >= self.params.exit_period {
            Some(self.calculate_lowest_low(candles, self.params.exit_period)?)
        } else {
            None
        };

        let atr = if candles.len() >= self.params.atr_period + 1 {
            Some(self.calculate_atr(candles, self.params.atr_period)?)
        } else {
            None
        };

        Ok(IndicatorSummary {
            current_price: latest.close,
            entry_high,
            entry_low,
            exit_high,
            exit_low,
            atr,
            timestamp: latest.timestamp,
        })
    }
}

/// 技术指标摘要
#[derive(Debug, Clone)]
pub struct IndicatorSummary {
    pub current_price: f64,       // 当前价格
    pub entry_high: Option<f64>,  // 入场最高价
    pub entry_low: Option<f64>,   // 入场最低价
    pub exit_high: Option<f64>,   // 离场最高价
    pub exit_low: Option<f64>,    // 离场最低价
    pub atr: Option<f64>,         // 平均真实波幅
    pub timestamp: DateTime<Utc>, // 时间戳
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_candles() -> Vec<CandleData> {
        let mut candles = Vec::new();
        let base_time = Utc::now();

        // 创建30根测试K线数据
        for i in 0..30 {
            candles.push(CandleData {
                timestamp: base_time + chrono::Duration::minutes(i),
                symbol: "SOL-USDT".to_string(),
                open: 100.0 + (i as f64 * 0.1),
                high: 102.0 + (i as f64 * 0.1),
                low: 98.0 + (i as f64 * 0.1),
                close: 101.0 + (i as f64 * 0.1),
                volume: 1000.0,
            });
        }

        candles
    }

    #[test]
    fn test_turtle_strategy_creation() {
        let strategy = TurtleStrategy::new("SOL-USDT".to_string(), None);
        assert_eq!(strategy.symbol, "SOL-USDT");
        assert_eq!(strategy.params.entry_period, 20);
    }

    #[test]
    fn test_atr_calculation() {
        let strategy = TurtleStrategy::new("SOL-USDT".to_string(), None);
        let candles = create_test_candles();

        let atr = strategy.calculate_atr(&candles, 10).unwrap();
        assert!(atr > 0.0);
    }

    #[test]
    fn test_highest_high_calculation() {
        let strategy = TurtleStrategy::new("SOL-USDT".to_string(), None);
        let candles = create_test_candles();

        let high = strategy.calculate_highest_high(&candles, 10).unwrap();
        assert!(high > 100.0);
    }
}
