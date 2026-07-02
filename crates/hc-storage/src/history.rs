use crate::schema;
use hc_core::model::*;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

#[cfg(test)]
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(test)]
static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct HistoryDb {
    conn: Mutex<Connection>,
}

impl HistoryDb {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        schema::initialize_db(&conn)?;
        Ok(HistoryDb {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_samples(&self, samples: &[Sample]) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("INSERT INTO samples (tag_id, ts, value, quality) VALUES (?1, ?2, ?3, ?4)")?;
        let mut count = 0;
        for s in samples {
            stmt.execute(params![
                s.tag_id,
                s.ts.timestamp_millis(),
                serde_json::to_string(&s.value).unwrap_or_default(),
                s.quality as i32
            ])?;
            count += 1;
        }
        Ok(count)
    }

    pub fn query_trend(
        &self,
        tag_ids: &[String],
        from_ms: i64,
        to_ms: i64,
        max: u32,
    ) -> rusqlite::Result<Vec<Sample>> {
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = tag_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT tag_id, ts, value, quality FROM samples WHERE tag_id IN ({}) AND ts >= ?{} AND ts <= ?{} ORDER BY ts ASC",
            placeholders.join(","), tag_ids.len()+1, tag_ids.len()+2);
        let mut stmt = conn.prepare(&sql)?;
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = tag_ids
            .iter()
            .map(|id| Box::new(id.clone()) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        params_vec.push(Box::new(from_ms));
        params_vec.push(Box::new(to_ms));
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            let tag_id: String = row.get(0)?;
            let ts_ms: i64 = row.get(1)?;
            let value_str: String = row.get(2)?;
            let qi: i32 = row.get(3)?;
            let value: Value = serde_json::from_str(&value_str).unwrap_or(Value::Bool(false));
            let quality = match qi {
                0 => Quality::Good,
                1 => Quality::Bad,
                _ => Quality::Stale,
            };
            Ok(Sample {
                tag_id,
                ts: chrono::DateTime::from_timestamp_millis(ts_ms).unwrap_or_default(),
                value,
                quality,
            })
        })?;
        let mut results: Vec<Sample> = rows.filter_map(|r| r.ok()).collect();
        if max > 0 && results.len() > max as usize {
            let step = results.len() / max as usize;
            results = results.into_iter().step_by(step.max(1)).collect();
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> HistoryDb {
        let dir = std::env::temp_dir().join("hc_test_hist");
        let _ = std::fs::create_dir_all(&dir);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = dir.join(format!("test_{}.db", id));
        let _ = std::fs::remove_file(&p);
        HistoryDb::open(&p).unwrap()
    }

    #[test]
    fn test_insert_and_query() {
        let db = temp_db();
        let now = chrono::Utc::now();
        db.insert_samples(&[
            Sample {
                tag_id: "d/t".into(),
                ts: now,
                value: Value::F32(25.5),
                quality: Quality::Good,
            },
            Sample {
                tag_id: "d/t".into(),
                ts: now + chrono::Duration::seconds(1),
                value: Value::F32(26.0),
                quality: Quality::Good,
            },
        ])
        .unwrap();
        let r = db
            .query_trend(&["d/t".into()], 0, 99999999999999, 100)
            .unwrap();
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn test_empty() {
        let db = temp_db();
        assert!(db
            .query_trend(&["x".into()], 0, 999, 100)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_downsample() {
        let db = temp_db();
        let now = chrono::Utc::now();
        let samples: Vec<Sample> = (0..100)
            .map(|i| Sample {
                tag_id: "d/t".into(),
                ts: now + chrono::Duration::milliseconds(i * 10),
                value: Value::F32(i as f32),
                quality: Quality::Good,
            })
            .collect();
        db.insert_samples(&samples).unwrap();
        let r = db
            .query_trend(&["d/t".into()], 0, 99999999999999, 10)
            .unwrap();
        assert!(r.len() <= 10 && !r.is_empty());
    }
}
