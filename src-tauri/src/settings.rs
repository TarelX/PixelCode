use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Locale {
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

pub(crate) fn read_locale(connection: &Connection) -> rusqlite::Result<Locale> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'locale'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(match stored.as_deref() {
        Some("en") => Locale::En,
        _ => Locale::default(),
    })
}

fn write_locale(connection: &Connection, locale: Locale) -> rusqlite::Result<()> {
    let value = match locale {
        Locale::ZhCn => "zh-CN",
        Locale::En => "en",
    };
    connection.execute(
        "INSERT INTO app_settings (key, value) VALUES ('locale', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [value],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_locale(connection: State<'_, Mutex<Connection>>) -> Result<Locale, String> {
    let connection = connection.lock().map_err(|error| error.to_string())?;
    read_locale(&connection).map_err(|error| error.to_string())
}

pub(crate) fn notifications_enabled(connection: &Connection) -> Result<bool, String> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'notifications_enabled'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(stored.as_deref() == Some("1"))
}

fn write_notifications_enabled(connection: &Connection, enabled: bool) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO app_settings (key, value) VALUES ('notifications_enabled', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [if enabled { "1" } else { "0" }],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_notifications_enabled(connection: State<'_, Mutex<Connection>>) -> Result<bool, String> {
    let connection = connection.lock().map_err(|error| error.to_string())?;
    notifications_enabled(&connection)
}

#[tauri::command]
pub fn set_notifications_enabled(
    connection: State<'_, Mutex<Connection>>,
    enabled: bool,
) -> Result<(), String> {
    let connection = connection.lock().map_err(|error| error.to_string())?;
    write_notifications_enabled(&connection, enabled).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_locale(connection: State<'_, Mutex<Connection>>, locale: Locale) -> Result<(), String> {
    let connection = connection.lock().map_err(|error| error.to_string())?;
    write_locale(&connection, locale).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_defaults_and_survives_reopening_the_database() {
        let directory =
            std::env::temp_dir().join(format!("pixelcode-settings-{}", std::process::id()));
        let connection = crate::db::open(&directory).unwrap();
        assert_eq!(read_locale(&connection).unwrap(), Locale::ZhCn);
        write_locale(&connection, Locale::En).unwrap();
        drop(connection);
        let connection = crate::db::open(&directory).unwrap();
        assert_eq!(read_locale(&connection).unwrap(), Locale::En);
        write_locale(&connection, Locale::ZhCn).unwrap();
        assert_eq!(read_locale(&connection).unwrap(), Locale::ZhCn);
        connection
            .execute(
                "UPDATE app_settings SET value = 'unknown' WHERE key = 'locale'",
                [],
            )
            .unwrap();
        assert_eq!(read_locale(&connection).unwrap(), Locale::ZhCn);
        assert!(!notifications_enabled(&connection).unwrap());
        write_notifications_enabled(&connection, true).unwrap();
        drop(connection);
        let connection = crate::db::open(&directory).unwrap();
        assert!(notifications_enabled(&connection).unwrap());
        drop(connection);
        let connection = crate::db::open(&directory).unwrap();
        write_notifications_enabled(&connection, false).unwrap();
        assert!(!notifications_enabled(&connection).unwrap());
        drop(connection);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
