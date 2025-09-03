use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{info, warn};
use rusqlite::{Connection, Row, params};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::types::{CandleData, TickerData, Trade, TradingSignal};

/// 数据库管理结构
pub struct Database {
    conn: Arc<Mutex<Connection>>, // 数据库连接
}

impl Database {
    /// 创建新的数据库实例
    pub async fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // 创建必要的表
        db.create_tables().await?;
        info!("数据库初始化完成: {}", db_path);

        Ok(db)
    }

    /// 创建数据表
    async fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().await;

        // 创建K线数据表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS candles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME NOT NULL,
                symbol TEXT NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume REAL NOT NULL,
                UNIQUE(timestamp, symbol)
            )",
            [],
        )?;

        // 创建Ticker数据表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tickers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME NOT NULL,
                symbol TEXT NOT NULL,
                last_price REAL NOT NULL,
                bid_price REAL NOT NULL,
                ask_price REAL NOT NULL,
                volume_24h REAL NOT NULL
            )",
            [],
        )?;

        // 创建交易信号表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS signals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME NOT NULL,
                symbol TEXT NOT NULL,
                signal_type TEXT NOT NULL,
                price REAL NOT NULL,
                strategy TEXT NOT NULL,
                reason TEXT NOT NULL,
                confidence REAL NOT NULL
            )",
            [],
        )?;

        // 创建交易记录表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS trades (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME NOT NULL,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                price REAL NOT NULL,
                quantity REAL NOT NULL,
                strategy TEXT NOT NULL,
                pnl REAL
            )",
            [],
        )?;

        // 创建索引以提高查询性能
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_candles_symbol_timestamp 
             ON candles(symbol, timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_trades_timestamp 
             ON trades(timestamp DESC)",
            [],
        )?;

        info!("数据库表创建完成");
        Ok(())
    }

    /// 保存K线数据
    pub async fn save_candle(&self, candle: &CandleData) -> Result<()> {
        let conn = self.conn.lock().await;

        match conn.execute(
            "INSERT OR REPLACE INTO candles 
             (timestamp, symbol, open, high, low, close, volume) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                candle.timestamp.timestamp_millis(),
                candle.symbol,
                candle.open,
                candle.high,
                candle.low,
                candle.close,
                candle.volume,
            ],
        ) {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("保存K线数据失败: {}", e);
                Err(e.into())
            }
        }
    }

    /// 批量保存K线数据
    pub async fn save_candles(&self, candles: &[CandleData]) -> Result<()> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO candles 
             (timestamp, symbol, open, high, low, close, volume) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        for candle in candles {
            stmt.execute(params![
                candle.timestamp.timestamp_millis(),
                candle.symbol,
                candle.open,
                candle.high,
                candle.low,
                candle.close,
                candle.volume,
            ])?;
        }

        info!("批量保存{}条K线数据", candles.len());
        Ok(())
    }

    /// 保存Ticker数据
    pub async fn save_ticker(&self, ticker: &TickerData) -> Result<()> {
        let conn = self.conn.lock().await;

        let last_price: f64 = ticker.last.parse().unwrap_or(0.0);
        let bid_price: f64 = ticker.bid_px.parse().unwrap_or(0.0);
        let ask_price: f64 = ticker.ask_px.parse().unwrap_or(0.0);
        let volume_24h: f64 = ticker.vol_ccy24h.parse().unwrap_or(0.0);
        let timestamp_ms: i64 = ticker.ts.parse().unwrap_or(0);

        conn.execute(
            "INSERT INTO tickers 
             (timestamp, symbol, last_price, bid_price, ask_price, volume_24h) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                timestamp_ms,
                ticker.inst_id,
                last_price,
                bid_price,
                ask_price,
                volume_24h,
            ],
        )?;

        Ok(())
    }

    /// 获取指定时间范围的K线数据
    pub async fn get_candles(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<CandleData>> {
        let conn = self.conn.lock().await;

        let mut query = format!(
            "SELECT timestamp, symbol, open, high, low, close, volume 
             FROM candles 
             WHERE symbol = ?1 AND timestamp >= ?2 AND timestamp <= ?3 
             ORDER BY timestamp ASC"
        );

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&query)?;
        let candle_iter = stmt.query_map(
            params![
                symbol,
                start_time.timestamp_millis(),
                end_time.timestamp_millis(),
            ],
            |row| {
                let timestamp_ms: i64 = row.get(0)?;
                let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

                Ok(CandleData {
                    timestamp,
                    symbol: row.get(1)?,
                    open: row.get(2)?,
                    high: row.get(3)?,
                    low: row.get(4)?,
                    close: row.get(5)?,
                    volume: row.get(6)?,
                })
            },
        )?;

        let mut candles = Vec::new();
        for candle in candle_iter {
            candles.push(candle?);
        }

        Ok(candles)
    }

    /// 获取最新的N条K线数据
    pub async fn get_latest_candles(&self, symbol: &str, count: usize) -> Result<Vec<CandleData>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT timestamp, symbol, open, high, low, close, volume 
             FROM candles 
             WHERE symbol = ?1 
             ORDER BY timestamp DESC 
             LIMIT ?2",
        )?;

        let candle_iter = stmt.query_map(params![symbol, count], |row| {
            let timestamp_ms: i64 = row.get(0)?;
            let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

            Ok(CandleData {
                timestamp,
                symbol: row.get(1)?,
                open: row.get(2)?,
                high: row.get(3)?,
                low: row.get(4)?,
                close: row.get(5)?,
                volume: row.get(6)?,
            })
        })?;

        let mut candles = Vec::new();
        for candle in candle_iter {
            candles.push(candle?);
        }

        // 按时间正序排列
        candles.reverse();
        Ok(candles)
    }

    /// 保存交易信号
    pub async fn save_signal(&self, signal: &TradingSignal) -> Result<()> {
        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT INTO signals 
             (timestamp, symbol, signal_type, price, strategy, reason, confidence) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                signal.timestamp.timestamp_millis(),
                signal.symbol,
                format!("{:?}", signal.signal_type),
                signal.price,
                signal.strategy,
                signal.reason,
                signal.confidence,
            ],
        )?;

        Ok(())
    }

    /// 保存交易记录
    pub async fn save_trade(&self, trade: &Trade) -> Result<i64> {
        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT INTO trades 
             (timestamp, symbol, side, price, quantity, strategy, pnl) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                trade.timestamp.timestamp_millis(),
                trade.symbol,
                trade.side,
                trade.price,
                trade.quantity,
                trade.strategy,
                trade.pnl,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// 获取最近的交易记录
    pub async fn get_recent_trades(&self, limit: usize) -> Result<Vec<Trade>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, symbol, side, price, quantity, strategy, pnl 
             FROM trades 
             ORDER BY timestamp DESC 
             LIMIT ?1",
        )?;

        let trade_iter = stmt.query_map(params![limit], |row| {
            let timestamp_ms: i64 = row.get(1)?;
            let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

            Ok(Trade {
                id: Some(row.get(0)?),
                timestamp,
                symbol: row.get(2)?,
                side: row.get(3)?,
                price: row.get(4)?,
                quantity: row.get(5)?,
                strategy: row.get(6)?,
                pnl: row.get(7)?,
            })
        })?;

        let mut trades = Vec::new();
        for trade in trade_iter {
            trades.push(trade?);
        }

        Ok(trades)
    }

    /// 获取指定时间范围内的交易记录
    pub async fn get_trades_by_time_range(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Trade>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, symbol, side, price, quantity, strategy, pnl 
             FROM trades 
             WHERE timestamp >= ?1 AND timestamp <= ?2 
             ORDER BY timestamp ASC",
        )?;

        let trade_iter = stmt.query_map(
            params![start_time.timestamp_millis(), end_time.timestamp_millis(),],
            |row| {
                let timestamp_ms: i64 = row.get(1)?;
                let timestamp = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_default();

                Ok(Trade {
                    id: Some(row.get(0)?),
                    timestamp,
                    symbol: row.get(2)?,
                    side: row.get(3)?,
                    price: row.get(4)?,
                    quantity: row.get(5)?,
                    strategy: row.get(6)?,
                    pnl: row.get(7)?,
                })
            },
        )?;

        let mut trades = Vec::new();
        for trade in trade_iter {
            trades.push(trade?);
        }

        Ok(trades)
    }

    /// 获取数据统计信息
    pub async fn get_stats(&self) -> Result<()> {
        let conn = self.conn.lock().await;

        let candle_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM candles", [], |row| row.get(0))?;

        let ticker_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM tickers", [], |row| row.get(0))?;

        let signal_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM signals", [], |row| row.get(0))?;

        let trade_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))?;

        info!("数据库统计:");
        info!("  K线数据: {} 条", candle_count);
        info!("  Ticker数据: {} 条", ticker_count);
        info!("  交易信号: {} 条", signal_count);
        info!("  交易记录: {} 条", trade_count);

        Ok(())
    }
}
