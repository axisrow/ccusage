use std::borrow::Cow;

pub(super) const CODEX_SOURCE_EXEC: &str = "Exec";
pub(super) const CODEX_SOURCE_UNCATEGORIZED: &str = "Uncategorized";

pub(super) fn normalize_codex_originator(originator: &str) -> Cow<'_, str> {
    let originator = originator.trim();
    match originator {
        "codex-tui" | "codex_cli_rs" => Cow::Borrowed("CLI"),
        "codex_exec" => Cow::Borrowed(CODEX_SOURCE_EXEC),
        "Codex Desktop" | "codex_work_desktop" => Cow::Borrowed("Desktop App"),
        "codex_vscode" => Cow::Borrowed("VS Code"),
        "codex_python_sdk" => Cow::Borrowed("SDK"),
        "" => Cow::Borrowed(CODEX_SOURCE_UNCATEGORIZED),
        // Originator is free-form: third-party clients record their own names,
        // so unknown values must pass through instead of being dropped.
        unknown => Cow::Borrowed(unknown),
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::normalize_codex_originator;

    #[test]
    fn maps_known_codex_originators_to_dashboard_labels() {
        let cases = [
            ("codex-tui", "CLI"),
            ("codex_cli_rs", "CLI"),
            ("codex_exec", "Exec"),
            ("Codex Desktop", "Desktop App"),
            ("codex_work_desktop", "Desktop App"),
            ("codex_vscode", "VS Code"),
            ("codex_python_sdk", "SDK"),
        ];
        for (originator, expected) in cases {
            assert_eq!(
                normalize_codex_originator(originator),
                Cow::Borrowed(expected),
                "originator {originator:?}"
            );
        }
    }

    #[test]
    fn passes_unknown_originators_through_unchanged() {
        for originator in ["Claude Code", "claudian", "probe", "My Custom Tool"] {
            assert_eq!(
                normalize_codex_originator(originator),
                Cow::Borrowed(originator),
                "originator {originator:?}"
            );
        }
    }

    #[test]
    fn maps_empty_originator_to_uncategorized() {
        assert_eq!(
            normalize_codex_originator(""),
            Cow::Borrowed("Uncategorized")
        );
    }

    #[test]
    fn trims_surrounding_whitespace_before_matching() {
        assert_eq!(
            normalize_codex_originator(" codex-tui "),
            Cow::Borrowed("CLI")
        );
    }
}
