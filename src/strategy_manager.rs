use crate::strategy::{Candle, Signal, SignalSide, Strategy};
use std::collections::HashMap;

pub struct StrategyManager {
    strategies: Vec<Box<dyn Strategy>>,
    // 每个标的维护自己的K线序列
    series: HashMap<String, Vec<Candle>>,
    keep: usize,
}

impl StrategyManager {
    pub fn new(keep: usize) -> Self {
        Self {
            strategies: Vec::new(),
            series: HashMap::new(),
            keep,
        }
    }

    pub fn add_strategy(&mut self, s: Box<dyn Strategy>) {
        self.strategies.push(s);
    }

    pub fn on_new_candle(&mut self, inst_id: &str, candle: Candle) {
        let entry = self.series.entry(inst_id.to_string()).or_default();
        entry.push(candle);
        if entry.len() > self.keep {
            let overflow = entry.len() - self.keep;
            entry.drain(0..overflow);
        }
        // 依次驱动全部策略
        let candles = entry.as_slice();

        for s in self.strategies.iter_mut() {
            if let Some(sig) = s.on_candle(inst_id, candles) {
                match sig.side {
                    SignalSide::Buy => {
                        println!(
                            "🟢 [{}][{}] BUY @ {:.4} ts={}",
                            inst_id,
                            s.name(),
                            sig.price,
                            sig.ts
                        );
                    }
                    SignalSide::Sell => {
                        println!(
                            "🔴 [{}][{}] SELL @ {:.4} ts={}",
                            inst_id,
                            s.name(),
                            sig.price,
                            sig.ts
                        );
                    }
                }
                // TODO: 这里可以把信号写库/发到执行引擎
            }
        }
    }
}
