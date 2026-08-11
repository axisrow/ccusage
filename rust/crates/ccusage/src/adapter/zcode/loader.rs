use std::path::Path;

use crate::{LoadedEntry, PricingMap, Result, cli::SharedArgs, debug_log, read_files_parallel};

use super::{
    parser::{ZCodeEntry, read_model_usage_row, to_loaded_entry},
    paths::zcode_db_paths,
};

const MODEL_USAGE_QUERY: &str = r#"
SELECT
    mu.id,
    mu.session_id,
    mu.model_id,
    mu.started_at,
    mu.input_tokens,
    mu.output_tokens,
    mu.reasoning_tokens,
    mu.cache_creation_input_tokens,
    mu.cache_read_input_tokens,
    s.directory
FROM model_usage AS mu
LEFT JOIN session AS s ON s.id = mu.session_id
WHERE mu.status = 'completed'
ORDER BY mu.started_at ASC, mu.id ASC
"#;

pub(crate) fn load_entries(shared: &SharedArgs, pricing: &PricingMap) -> Result<Vec<LoadedEntry>> {
    crate::progress::track_usage_load(crate::progress::UsageLoadAgent::ZCode, shared.json, || {
        load_entries_inner(shared, pricing)
    })
}

fn load_entries_inner(shared: &SharedArgs, pricing: &PricingMap) -> Result<Vec<LoadedEntry>> {
    let tz = crate::parse_tz(shared.timezone.as_deref());
    let db_paths = zcode_db_paths()?;
    let loaded = read_files_parallel(&db_paths, shared.single_thread, |db_path| {
        load_model_usage_entries(db_path, shared)
    });
    let mut entries = loaded
        .into_iter()
        .flatten()
        .map(|entry| to_loaded_entry(entry, tz.as_ref(), shared.mode, pricing))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.timestamp);
    Ok(entries)
}

fn load_model_usage_entries(db_path: &Path, shared: &SharedArgs) -> Vec<ZCodeEntry> {
    let Ok(connection) =
        sqlite::Connection::open_with_flags(db_path, sqlite::OpenFlags::new().with_read_only())
    else {
        debug_log(
            shared,
            format!("Failed to open ZCode database: {}", db_path.display()),
        );
        return Vec::new();
    };
    let Ok(mut statement) = connection.prepare(MODEL_USAGE_QUERY) else {
        debug_log(
            shared,
            format!("Failed to read ZCode database: {}", db_path.display()),
        );
        return Vec::new();
    };
    let mut entries = Vec::new();
    loop {
        match statement.next() {
            Ok(sqlite::State::Row) => {
                if let Some(entry) = read_model_usage_row(&statement) {
                    entries.push(entry);
                }
            }
            Ok(sqlite::State::Done) => break,
            Err(_) => {
                debug_log(
                    shared,
                    format!("Failed to query ZCode database: {}", db_path.display()),
                );
                break;
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::cli::SharedArgs;
    use ccusage_test_support::fs_fixture;

    fn create_zcode_db(path: &Path) {
        let db = sqlite::open(path).unwrap();
        db.execute(
            "
            CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);
            CREATE TABLE model_usage (
                id INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL,
                model_id TEXT,
                status TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                input_tokens INTEGER,
                output_tokens INTEGER,
                reasoning_tokens INTEGER,
                cache_creation_input_tokens INTEGER,
                cache_read_input_tokens INTEGER
            );
            ",
        )
        .unwrap();
    }

    #[test]
    fn loads_completed_rows_and_excludes_non_completed_rows() {
        let fixture = fs_fixture!({});
        let db_path = fixture.path("db.sqlite");
        create_zcode_db(&db_path);
        let db = sqlite::open(&db_path).unwrap();
        db.execute("INSERT INTO session (id, directory) VALUES ('session-1', '/workspace/zcode')")
            .unwrap();
        db.execute("INSERT INTO model_usage VALUES (1, 'session-1', 'GLM-5.2', 'completed', 1735689600123, 1000, 300, 50, 100, 200)")
            .unwrap();
        db.execute("INSERT INTO model_usage VALUES (2, 'session-1', 'GLM-5.2', 'running', 1735689601123, 900, 200, 0, 0, 0)")
            .unwrap();
        drop(db);

        let shared = SharedArgs {
            timezone: Some("UTC".to_string()),
            ..SharedArgs::default()
        };
        let pricing = PricingMap::load_embedded();
        let entries = load_model_usage_entries(&db_path, &shared)
            .into_iter()
            .map(|entry| {
                to_loaded_entry(
                    entry,
                    crate::parse_tz(Some("UTC")).as_ref(),
                    crate::cli::CostMode::Auto,
                    &pricing,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, "2025-01-01");
        assert_eq!(entries[0].data.message.usage.input_tokens, 700);
        assert_eq!(entries[0].data.message.usage.cache_read_input_tokens, 200);
        assert_eq!(
            entries[0].data.message.usage.cache_creation_input_tokens,
            100
        );
        assert_eq!(entries[0].extra_total_tokens, 0);
        assert_eq!(entries[0].project_path.as_ref(), "/workspace/zcode");
    }

    #[test]
    #[ignore = "requires the local ZCode SQLite database"]
    fn loads_local_zcode_database() {
        let shared = SharedArgs::default();
        let pricing = PricingMap::load_embedded();
        assert!(!load_entries(&shared, &pricing).unwrap().is_empty());
    }
}
