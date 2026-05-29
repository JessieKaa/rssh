use rusqlite::params;

use super::Db;
use crate::error::AppResult;
use crate::models::{validate_name, FrpcConfig};

fn row_to_config(row: &rusqlite::Row) -> rusqlite::Result<FrpcConfig> {
    Ok(FrpcConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        file_name: row.get(2)?,
        enabled: row.get::<_, i32>(3)? != 0,
        created_at: row.get(4)?,
    })
}

pub fn list(db: &Db) -> AppResult<Vec<FrpcConfig>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, file_name, enabled, created_at FROM frpc_configs ORDER BY name ASC",
    )?;
    let rows = stmt.query_map([], row_to_config)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get(db: &Db, id: &str) -> AppResult<FrpcConfig> {
    let conn = db.lock()?;
    conn.query_row(
        "SELECT id, name, file_name, enabled, created_at FROM frpc_configs WHERE id = ?1",
        params![id],
        row_to_config,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            crate::error::AppError::not_found("frpc_config_not_found", serde_json::json!({}))
        }
        other => other.into(),
    })
}

pub fn insert(db: &Db, c: &FrpcConfig) -> AppResult<()> {
    validate_name(&c.name)?;
    let conn = db.lock()?;
    conn.execute(
        "INSERT INTO frpc_configs (id, name, file_name, enabled, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, file_name=excluded.file_name, \
         enabled=excluded.enabled, created_at=excluded.created_at",
        params![c.id, c.name, c.file_name, c.enabled as i32, c.created_at],
    )?;
    Ok(())
}

pub fn update(db: &Db, c: &FrpcConfig) -> AppResult<()> {
    validate_name(&c.name)?;
    let conn = db.lock()?;
    conn.execute(
        "UPDATE frpc_configs SET name=?1, file_name=?2, enabled=?3 WHERE id=?4",
        params![c.name, c.file_name, c.enabled as i32, c.id],
    )?;
    Ok(())
}

pub fn delete(db: &Db, id: &str) -> AppResult<()> {
    let conn = db.lock()?;
    conn.execute("DELETE FROM frpc_configs WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn list_enabled(db: &Db) -> AppResult<Vec<FrpcConfig>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, file_name, enabled, created_at FROM frpc_configs WHERE enabled = 1 ORDER BY name ASC",
    )?;
    let rows = stmt.query_map([], row_to_config)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn toggle_enabled(db: &Db, id: &str, enabled: bool) -> AppResult<()> {
    let conn = db.lock()?;
    conn.execute(
        "UPDATE frpc_configs SET enabled = ?1 WHERE id = ?2",
        params![enabled as i32, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: &str, name: &str, file_name: &str) -> FrpcConfig {
        FrpcConfig {
            id: id.into(),
            name: name.into(),
            file_name: file_name.into(),
            enabled: false,
            created_at: 0,
        }
    }

    #[test]
    fn insert_then_get() {
        let db = Db::open_in_memory().unwrap();
        insert(&db, &mk("c1", "my-server", "abc.toml")).unwrap();
        let got = get(&db, "c1").unwrap();
        assert_eq!(got.name, "my-server");
        assert_eq!(got.file_name, "abc.toml");
        assert!(!got.enabled);
    }

    #[test]
    fn upsert_overwrites() {
        let db = Db::open_in_memory().unwrap();
        insert(&db, &mk("c1", "old", "a.toml")).unwrap();
        let mut updated = mk("c1", "new", "a.toml");
        updated.enabled = true;
        insert(&db, &updated).unwrap();
        let got = get(&db, "c1").unwrap();
        assert_eq!(got.name, "new");
        assert!(got.enabled);
    }

    #[test]
    fn list_sorted_by_name() {
        let db = Db::open_in_memory().unwrap();
        insert(&db, &mk("c1", "zebra", "z.toml")).unwrap();
        insert(&db, &mk("c2", "apple", "a.toml")).unwrap();
        let names: Vec<String> = list(&db).unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["apple", "zebra"]);
    }

    #[test]
    fn delete_removes_row() {
        let db = Db::open_in_memory().unwrap();
        insert(&db, &mk("c1", "test", "t.toml")).unwrap();
        delete(&db, "c1").unwrap();
        assert_eq!(get(&db, "c1").unwrap_err().code(), "frpc_config_not_found");
    }

    #[test]
    fn list_enabled_filters() {
        let db = Db::open_in_memory().unwrap();
        insert(&db, &mk("c1", "disabled", "d.toml")).unwrap();
        let mut enabled_cfg = mk("c2", "enabled", "e.toml");
        enabled_cfg.enabled = true;
        insert(&db, &enabled_cfg).unwrap();
        let enabled = list_enabled(&db).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "enabled");
    }

    #[test]
    fn toggle_enabled_works() {
        let db = Db::open_in_memory().unwrap();
        insert(&db, &mk("c1", "test", "t.toml")).unwrap();
        toggle_enabled(&db, "c1", true).unwrap();
        assert!(get(&db, "c1").unwrap().enabled);
        toggle_enabled(&db, "c1", false).unwrap();
        assert!(!get(&db, "c1").unwrap().enabled);
    }

    #[test]
    fn insert_rejects_control_char_name() {
        let db = Db::open_in_memory().unwrap();
        let bad = mk("c1", "bad\nname", "t.toml");
        assert_eq!(
            insert(&db, &bad).unwrap_err().code(),
            "name_has_control_char"
        );
    }
}
