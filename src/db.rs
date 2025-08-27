use lazy_static::lazy_static;
use rusqlite::{Connection, Result, params};
use std::sync::Mutex;

lazy_static! {
    pub static ref DB: Mutex<Connection> = {
        let conn = Connection::open("okx_data.db").unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ticker (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                inst_id TEXT,
                price REAL,
                ts DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS candle (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                inst_id TEXT,
                open REAL,
                high REAL,
                low REAL,
                close REAL,
                ts DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .unwrap();
        Mutex::new(conn)
    };
}

// 保存 ticker
pub fn save_ticker(inst_id: &str, price: f64) -> Result<()> {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT INTO ticker (inst_id, price) VALUES (?1, ?2)",
        params![inst_id, price],
    )?;
    Ok(())
}

// 保存 candle
pub fn save_candle(inst_id: &str, open: f64, high: f64, low: f64, close: f64) -> Result<()> {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT INTO candle (inst_id, open, high, low, close) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![inst_id, open, high, low, close],
    )?;
    Ok(())
}
