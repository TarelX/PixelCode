use std::path::Path;

use rusqlite::{Connection, TransactionBehavior};

const MIGRATIONS: &[&str] = &[include_str!("../migrations/001_initial.sql")];

pub fn open(data_dir: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(data_dir)
        .map_err(|error| format!("Unable to create application data directory: {error}"))?;
    let mut connection = Connection::open(data_dir.join("pixelcode.db"))
        .map_err(|error| format!("Unable to open PixelCode database: {error}"))?;
    migrate(&mut connection)
        .map_err(|error| format!("Unable to migrate PixelCode database: {error}"))?;
    Ok(connection)
}

fn migrate(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: usize = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > MIGRATIONS.len() {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "Database schema version {version} is newer than supported version {}",
            MIGRATIONS.len()
        )));
    }
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version) {
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", index + 1)?;
    }
    transaction.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(connection: &Connection) -> usize {
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn migrations_create_all_tables() {
        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(version(&connection), 0);
        migrate(&mut connection).unwrap();
        assert_eq!(version(&connection), MIGRATIONS.len());
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            tables,
            [
                "app_settings",
                "artifacts",
                "camps",
                "messages",
                "plans",
                "roles",
                "tasks"
            ]
        );
    }

    #[test]
    fn rerunning_migrations_is_a_no_op() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        connection
            .execute(
                "INSERT INTO camps (id, root_path) VALUES ('camp', '/project')",
                [],
            )
            .unwrap();
        let schema_version: i64 = connection
            .pragma_query_value(None, "schema_version", |row| row.get(0))
            .unwrap();
        migrate(&mut connection).unwrap();
        assert_eq!(version(&connection), MIGRATIONS.len());
        assert_eq!(
            connection
                .pragma_query_value::<i64, _>(None, "schema_version", |row| row.get(0))
                .unwrap(),
            schema_version
        );
        let path: String = connection
            .query_row("SELECT root_path FROM camps WHERE id = 'camp'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(path, "/project");
    }

    #[test]
    fn failed_migration_rolls_back_schema_and_version() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE roles (id TEXT);")
            .unwrap();
        assert!(migrate(&mut connection).is_err());
        assert_eq!(version(&connection), 0);
        let camps: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'camps'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(camps, 0);
    }

    #[test]
    fn newer_database_is_rejected_without_downgrading() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", 999).unwrap();
        assert!(migrate(&mut connection).is_err());
        assert_eq!(version(&connection), 999);
    }
}
