use std::{collections::HashSet, env, path::PathBuf};

use crate::Result;

pub(super) const ZCODE_HOME_ENV: &str = "ZCODE_HOME";
pub(super) const ZCODE_DB_RELATIVE_PATH: &str = "cli/db/db.sqlite";

pub(super) fn zcode_db_paths() -> Result<Vec<PathBuf>> {
    let homes = match env::var(ZCODE_HOME_ENV) {
        Ok(value) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .collect(),
        Err(_) => {
            let home = crate::home::home_dir()
                .ok_or_else(|| crate::cli_error("home directory is not set"))?;
            vec![home.join(".zcode")]
        }
    };
    let mut seen = HashSet::new();
    Ok(homes
        .into_iter()
        .map(|home| home.join(ZCODE_DB_RELATIVE_PATH))
        .filter(|path| path.is_file())
        .filter(|path| seen.insert(path.clone()))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use ccusage_test_support::{EnvVarGuard, fs_fixture};

    #[test]
    fn discovers_database_under_zcode_home() {
        let fixture = fs_fixture!({
            "cli/db/db.sqlite": "",
        });
        let _env = EnvVarGuard::set(ZCODE_HOME_ENV, fixture.root());

        let paths = zcode_db_paths().unwrap();

        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with(Path::new(ZCODE_DB_RELATIVE_PATH)));
    }
}
