use chrono::Utc;
use lazy_static::lazy_static;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Candle {
    pub ts: i64, // 毫秒时间戳
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum SignalSide {
    Buy,
    Sell,
}

pub struct Signal {
    pub side: SignalSide,
    pub reason: &'static str,
    pub price: f64,
    pub ts: i64,
}

pub trait Strategy: Send {
    /// 新K线到达时被调用；若有信号，返回 Some(Signal)
    fn on_candle(&mut self, inst_id: &str, candles: &[Candle]) -> Option<Signal>;
    /// 策略名（用于日志）
    fn name(&self) -> &'static str;
}

/* -------------------------
   MC 策略（由你提供的EasyLanguage策略改写）
   参数：KC, KS, pls, ply, TT
------------------------- */
pub struct McStrategy {
    kc: f64,
    ks: usize,
    pls: f64,
    ply: f64,
    tt: usize,
    // 记录入场用
    entry_price: f64,
    // 记录触发bar（用K线序号理解）
    var6_index: Option<usize>,
}

impl McStrategy {
    pub fn new(kc: f64, ks: usize, pls: f64, ply: f64, tt: usize) -> Self {
        Self {
            kc,
            ks,
            pls,
            ply,
            tt,
            entry_price: 0.0,
            var6_index: None,
        }
    }
}

impl Strategy for McStrategy {
    fn on_candle(&mut self, _inst_id: &str, candles: &[Candle]) -> Option<Signal> {
        let n = candles.len();
        if n <= self.ks + 1 {
            return None;
        }

        let cur = &candles[n - 1];
        let prev = &candles[n - 2];
        let ks_bar = &candles[n - 1 - self.ks];

        // 对应源码：
        // var0=maxlist(open[KS],close[KS]);
        // var1=minlist(open[KS],close[KS]);
        // var2=maxlist(open[1],close[1]);
        // var3=minlist(open[1],close[1]);
        let var0 = ks_bar.open.max(ks_bar.close);
        let var1 = ks_bar.open.min(ks_bar.close);
        let var2 = prev.open.max(prev.close);
        let var3 = prev.open.min(prev.close);

        let condition1 = var3 > var0;
        let condition2 = cur.close < cur.open;
        let condition3 = cur.close < var1;
        let condition4 = cur.open > var2;
        let condition5 = (cur.open - cur.close).abs() < self.kc * cur.close;

        let condition6 = var1 > var2;
        let condition7 = cur.close > cur.open;
        let condition8 = cur.open < var3;
        let condition9 = cur.close > var0;
        let condition10 = (cur.close - cur.open).abs() < self.kc * cur.close;

        // 卖出开仓（sellshort next bar at market），此处在现K线收盘时产生信号
        if condition1 && condition2 && condition3 && condition4 && condition5 {
            self.entry_price = cur.close;
            self.var6_index = Some(n); // 当前K线索引（1-based类似）
            return Some(Signal {
                side: SignalSide::Sell,
                reason: "MC short entry",
                price: cur.close,
                ts: cur.ts,
            });
        }

        // 买入开仓（buy next bar at market）
        if condition6 && condition7 && condition8 && condition9 && condition10 {
            self.entry_price = cur.close;
            self.var6_index = Some(n);
            return Some(Signal {
                side: SignalSide::Buy,
                reason: "MC long entry",
                price: cur.close,
                ts: cur.ts,
            });
        }

        // 止盈/止损/超时离场（原策略里是用 next bar stop/limit，这里仅打印提示）
        if let Some(start) = self.var6_index {
            // 当前bar序号大于入场bar，开始计算出场位
            if n > start {
                let value1 = (1.0 - self.pls) * self.entry_price; // 多单止损，或空单止盈
                let value2 = (1.0 + self.ply) * self.entry_price; // 多单止盈，或空单止损
                let value3 = (1.0 + self.pls) * self.entry_price; // 空单止损，或多单止盈
                let value4 = (1.0 - self.ply) * self.entry_price; // 空单止盈，或多单止损

                // 这里只打印：真实下单请在执行层实现
                println!(
                    "[{}][MC] exit levels: v1={:.4}, v2={:.4}, v3={:.4}, v4={:.4}",
                    Utc::now(),
                    value1,
                    value2,
                    value3,
                    value4
                );

                // 超时TT根后按入场价离场（这里打印提示）
                if n >= start + self.tt {
                    println!(
                        "[{}][MC] timeout exit at entry price {:.4}",
                        Utc::now(),
                        self.entry_price
                    );
                    // 真实实现时在此返回平仓信号；此处仅信息提示
                }
            }
        }

        None
    }

    fn name(&self) -> &'static str {
        "MC"
    }
}

/* -------------------------
   演示用 双均线策略（MA交叉）
------------------------- */
pub struct MaCrossStrategy {
    short: usize,
    long: usize,
    last_state: i8, // -1:空方; 0:未知; 1:多方
}

impl MaCrossStrategy {
    pub fn new(short: usize, long: usize) -> Self {
        Self {
            short,
            long,
            last_state: 0,
        }
    }
}

impl Strategy for MaCrossStrategy {
    fn on_candle(&mut self, _inst_id: &str, candles: &[Candle]) -> Option<Signal> {
        let n = candles.len();
        if n < self.long {
            return None;
        }

        let sma = |k: usize| -> f64 {
            let mut s = 0.0;
            for c in &candles[n - k..] {
                s += c.close;
            }
            s / (k as f64)
        };
        let ma_s = sma(self.short);
        let ma_l = sma(self.long);

        let state = if ma_s > ma_l {
            1
        } else if ma_s < ma_l {
            -1
        } else {
            0
        };

        // 只在状态切换时发信号
        if state != 0 && state != self.last_state {
            self.last_state = state;
            let side = if state > 0 {
                SignalSide::Buy
            } else {
                SignalSide::Sell
            };
            let cur = &candles[n - 1];
            return Some(Signal {
                side,
                reason: "MA cross",
                price: cur.close,
                ts: cur.ts,
            });
        }
        None
    }

    fn name(&self) -> &'static str {
        "MA-Cross"
    }
}
