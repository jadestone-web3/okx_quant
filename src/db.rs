use lazy_static::lazy_static;
use rusqlite::{Connection, Result, params};
use std::sync::Mutex;

lazy_static! {
    static ref CONN: Mutex<Connection> = {
        let conn = Connection::open("okx_data.db").expect("open sqlite failed");
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS ticker (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                inst_id TEXT NOT NULL,
                price   REAL NOT NULL,
                ts_ms   INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS candle (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                inst_id TEXT NOT NULL,
                ts_ms   INTEGER NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low  REAL NOT NULL,
                close REAL NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .unwrap();
        Mutex::new(conn)
    };
}

pub fn save_ticker(inst_id: &str, price: f64, ts_ms: i64) -> Result<()> {
    let conn = CONN.lock().unwrap();
    conn.execute(
        "INSERT INTO ticker(inst_id, price, ts_ms) VALUES (?1, ?2, ?3)",
        params![inst_id, price, ts_ms],
    )?;
    Ok(())
}

pub fn save_candle(inst_id: &str, ts_ms: i64, o: f64, h: f64, l: f64, c: f64) -> Result<()> {
    let conn = CONN.lock().unwrap();
    conn.execute(
        "INSERT INTO candle(inst_id, ts_ms, open, high, low, close) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![inst_id, ts_ms, o, h, l, c],
    )?;
    Ok(())
}
