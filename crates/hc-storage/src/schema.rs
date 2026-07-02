use rusqlite::Connection;

pub fn initialize_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS samples (
            tag_id TEXT NOT NULL, ts INTEGER NOT NULL,
            value TEXT NOT NULL, quality INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tag_ts ON samples(tag_id, ts DESC);
        CREATE TABLE IF NOT EXISTS alarms (
            id INTEGER PRIMARY KEY AUTOINCREMENT, tag_id TEXT NOT NULL,
            ts INTEGER NOT NULL, level INTEGER DEFAULT 0,
            condition TEXT, value TEXT, ack INTEGER DEFAULT 0
        );",
    )?;
    Ok(())
}
