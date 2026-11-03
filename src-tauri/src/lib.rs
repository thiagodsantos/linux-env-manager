mod commands;
mod domain;
mod models;
mod shell;
mod state;
mod storage;

pub use domain::valid_key;
pub use models::{
    ImportPreview, ImportResult, InvalidImportLine, SaveVariableInput, Shell, ShellIntegration,
    Variable,
};

pub fn run() {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--print-exports")) {
        let Some(path) = arguments.next() else {
            eprintln!("A variable store path is required.");
            std::process::exit(2);
        };
        if arguments.next().is_some() {
            eprintln!("Unexpected arguments for export mode.");
            std::process::exit(2);
        }
        match storage::load_store(std::path::Path::new(&path)) {
            Ok(variables) => {
                for (key, value) in variables {
                    println!("export {key}={}", shell::shell_quote(&value));
                }
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    commands::run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{apply_delete, apply_save, merge_import, parse_env};
    use crate::models::PendingImport;
    use crate::shell::{
        managed_block_range, managed_shell_block, replace_managed_block, shell_quote,
        SHELL_BLOCK_END, SHELL_BLOCK_START,
    };
    use crate::storage::{
        atomic_write_using, clear_variables_with_test_key, commit_import_at_with_test_key,
        load_store_with_test_key, save_store_with_test_key, STORE_FILE,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use tempfile::tempdir;

    const TEST_ENCRYPTION_KEY: [u8; 32] = [42; 32];

    #[test]
    fn validates_environment_variable_names() {
        for key in ["A", "_private", "PATH", "HOME_2"] {
            assert!(valid_key(key), "{key} should be valid");
        }
        for key in ["", "2FAST", "HAS-DASH", "WITH SPACE", "é"] {
            assert!(!valid_key(key), "{key} should be invalid");
        }
    }

    #[test]
    fn rejects_nul_values_in_crud_and_import() {
        let mut variables = BTreeMap::new();
        let saved = apply_save(
            &mut variables,
            SaveVariableInput {
                key: "TOKEN".to_string(),
                value: "before\0after".to_string(),
                original_key: None,
            },
        );
        assert!(saved.is_err());
        assert!(variables.is_empty());

        let parsed = parse_env("TOKEN=before\0after\nGOOD=value\n");
        assert_eq!(parsed.invalid_lines.len(), 1);
        assert_eq!(
            parsed.variables.get("GOOD").map(String::as_str),
            Some("value")
        );
        assert!(!parsed.invalid_lines[0].message.contains("before"));
    }

    #[test]
    fn shell_quoting_handles_spaces_quotes_and_shell_syntax() {
        assert_eq!(shell_quote("plain/path"), "plain/path");
        assert_eq!(
            shell_quote("a'b $(touch unsafe)"),
            "'a'\\''b $(touch unsafe)'"
        );
    }

    #[test]
    fn generated_shell_block_reads_the_store_and_quotes_its_path() {
        let block = managed_shell_block(
            Path::new("/tmp/app's data/variables.json"),
            Path::new("/opt/Environment Manager/bin/linux-env-manager"),
        )
        .unwrap();
        assert!(block.starts_with("# >>> linux-env-manager managed shell integration >>>\n"));
        assert!(block.contains("--print-exports"));
        assert!(block.contains("/tmp/app'\\''s data/variables.json"));
        assert!(block.contains("/opt/Environment Manager/bin/linux-env-manager"));
        assert!(!block.contains("python3"));
        assert!(block.ends_with("# <<< linux-env-manager managed shell integration <<<\n"));
    }

    #[test]
    fn shell_integration_contract_uses_the_required_names() {
        let integrations = vec![
            ShellIntegration {
                shell: Shell::Bash,
                enabled: true,
                startup_file: Shell::Bash.startup_file(),
            },
            ShellIntegration {
                shell: Shell::Zsh,
                enabled: false,
                startup_file: Shell::Zsh.startup_file(),
            },
        ];
        assert_eq!(
            serde_json::to_value(integrations).unwrap(),
            serde_json::json!([
                { "shell": "bash", "enabled": true, "startupFile": "~/.bashrc" },
                { "shell": "zsh", "enabled": false, "startupFile": "~/.zshrc" }
            ])
        );
    }

    #[test]
    fn managed_block_add_update_and_removal_preserve_unrelated_bytes() {
        let original = b"# user config\nexport BEFORE=1\n";
        let first = b"# >>> linux-env-manager managed shell integration >>>\nfirst\n# <<< linux-env-manager managed shell integration <<<\n";
        let second = b"# >>> linux-env-manager managed shell integration >>>\nsecond\n# <<< linux-env-manager managed shell integration <<<\n";
        let added = replace_managed_block(original, Some(first)).unwrap();
        assert!(added.starts_with(original));
        let updated = replace_managed_block(&added, Some(second)).unwrap();
        let mut expected = original.to_vec();
        expected.extend_from_slice(second);
        assert_eq!(updated, expected);
        let removed = replace_managed_block(&updated, None).unwrap();
        assert_eq!(removed, original);
    }

    #[test]
    fn malformed_managed_markers_are_not_modified() {
        let partial = b"# >>> linux-env-manager managed shell integration >>>\n";
        assert!(replace_managed_block(partial, None).is_err());
        let mut duplicate = Vec::new();
        duplicate.extend_from_slice(SHELL_BLOCK_START);
        duplicate.extend_from_slice(b"\n");
        duplicate.extend_from_slice(SHELL_BLOCK_START);
        duplicate.extend_from_slice(b"\n");
        duplicate.extend_from_slice(SHELL_BLOCK_END);
        duplicate.extend_from_slice(b"\n");
        assert!(managed_block_range(&duplicate).is_err());
    }

    #[test]
    fn parses_comments_export_quotes_and_values_with_equals() {
        let parsed =
            parse_env("# comment\n\nA=one=two\nexport B='quoted value'\nC=\"$A\"\nEMPTY=\"\"\n");
        assert!(parsed.invalid_lines.is_empty());
        assert_eq!(parsed.variables["A"], "one=two");
        assert_eq!(parsed.variables["B"], "quoted value");
        assert_eq!(parsed.variables["C"], "$A");
        assert_eq!(parsed.variables["EMPTY"], "");
    }

    #[test]
    fn parser_reports_bad_lines_without_echoing_their_values() {
        let parsed = parse_env("NOT AN ASSIGNMENT\nBAD-KEY=secret-value\nOPEN=\"unfinished\n");
        assert_eq!(parsed.invalid_lines.len(), 3);
        assert_eq!(parsed.invalid_lines[0].line, 1);
        assert!(!parsed
            .invalid_lines
            .iter()
            .any(|line| line.message.contains("secret-value")));
    }

    #[test]
    fn last_duplicate_assignment_wins() {
        let parsed = parse_env("TOKEN=first\nTOKEN=last\n");
        assert!(parsed.invalid_lines.is_empty());
        assert_eq!(parsed.variables["TOKEN"], "last");
    }

    #[test]
    fn save_rejects_duplicates_and_supports_update_and_rename() {
        let mut variables = BTreeMap::from([
            ("FIRST".to_string(), "one".to_string()),
            ("SECOND".to_string(), "two".to_string()),
        ]);
        assert!(apply_save(
            &mut variables,
            SaveVariableInput {
                key: "SECOND".to_string(),
                value: "overwrite".to_string(),
                original_key: None,
            },
        )
        .is_err());
        apply_save(
            &mut variables,
            SaveVariableInput {
                key: "FIRST".to_string(),
                value: "updated".to_string(),
                original_key: Some("FIRST".to_string()),
            },
        )
        .unwrap();
        assert_eq!(variables["FIRST"], "updated");
        assert!(apply_save(
            &mut variables,
            SaveVariableInput {
                key: "SECOND".to_string(),
                value: "collision".to_string(),
                original_key: Some("FIRST".to_string()),
            },
        )
        .is_err());
        apply_save(
            &mut variables,
            SaveVariableInput {
                key: "RENAMED".to_string(),
                value: "renamed".to_string(),
                original_key: Some("FIRST".to_string()),
            },
        )
        .unwrap();
        assert!(!variables.contains_key("FIRST"));
        assert_eq!(variables["RENAMED"], "renamed");
    }

    #[test]
    fn delete_requires_an_existing_key() {
        let mut variables = BTreeMap::from([("A".to_string(), "value".to_string())]);
        apply_delete(&mut variables, "A").unwrap();
        assert!(variables.is_empty());
        assert!(apply_delete(&mut variables, "MISSING").is_err());
    }

    #[test]
    fn clearing_variables_persists_an_empty_app_store() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let original = BTreeMap::from([
            ("TOKEN".to_string(), "secret".to_string()),
            ("HOME".to_string(), "/home/tester".to_string()),
        ]);
        save_store_with_test_key(&path, &original, &TEST_ENCRYPTION_KEY).unwrap();

        clear_variables_with_test_key(&path, &TEST_ENCRYPTION_KEY).unwrap();

        assert!(load_store_with_test_key(&path, &TEST_ENCRYPTION_KEY)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn import_merges_existing_values_and_adds_new_keys() {
        let current = BTreeMap::from([("EXISTING".to_string(), "old".to_string())]);
        let incoming = BTreeMap::from([
            ("EXISTING".to_string(), "new".to_string()),
            ("NEW".to_string(), "value".to_string()),
        ]);
        let (merged, result) = merge_import(&current, &incoming);
        assert_eq!(merged["EXISTING"], "new");
        assert_eq!(merged["NEW"], "value");
        assert_eq!(result.added, vec!["NEW"]);
        assert_eq!(result.updated, vec!["EXISTING"]);
    }

    #[test]
    fn import_commit_atomically_persists_added_and_updated_keys() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let original = BTreeMap::from([("EXISTING".to_string(), "old".to_string())]);
        save_store_with_test_key(&path, &original, &TEST_ENCRYPTION_KEY).unwrap();
        let parsed = parse_env("EXISTING=new\nNEW=value\n");
        let batch = PendingImport {
            variables: parsed.variables,
            invalid_lines: parsed.invalid_lines,
        };
        let result = commit_import_at_with_test_key(&path, &batch, &TEST_ENCRYPTION_KEY).unwrap();
        assert_eq!(result.added, vec!["NEW"]);
        assert_eq!(result.updated, vec!["EXISTING"]);
        let persisted = load_store_with_test_key(&path, &TEST_ENCRYPTION_KEY).unwrap();
        assert_eq!(persisted["EXISTING"], "new");
        assert_eq!(persisted["NEW"], "value");
    }

    #[test]
    fn store_round_trips_and_uses_restrictive_permissions() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let variables = BTreeMap::from([("TOKEN".to_string(), "secret".to_string())]);
        save_store_with_test_key(&path, &variables, &TEST_ENCRYPTION_KEY).unwrap();
        assert_eq!(
            load_store_with_test_key(&path, &TEST_ENCRYPTION_KEY).unwrap(),
            variables
        );
        let file = fs::read_to_string(&path).unwrap();
        assert!(!file.contains("secret"));
        assert!(file.contains("AES-256-GCM"));
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn legacy_plaintext_store_is_migrated_to_encrypted_json() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        fs::write(
            &path,
            r#"{"version":1,"variables":{"TOKEN":"legacy-secret"}}"#,
        )
        .unwrap();

        assert_eq!(
            load_store_with_test_key(&path, &TEST_ENCRYPTION_KEY).unwrap()["TOKEN"],
            "legacy-secret"
        );
        let encrypted = fs::read_to_string(&path).unwrap();
        assert!(encrypted.contains("AES-256-GCM"));
        assert!(!encrypted.contains("legacy-secret"));
    }

    #[test]
    fn encrypted_store_rejects_a_wrong_key_without_disclosing_values() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        save_store_with_test_key(
            &path,
            &BTreeMap::from([("TOKEN".to_string(), "secret-value".to_string())]),
            &TEST_ENCRYPTION_KEY,
        )
        .unwrap();

        let error = load_store_with_test_key(&path, &[7; 32]).unwrap_err();
        assert!(error.contains("Unable to decrypt"));
        assert!(!error.contains("secret-value"));
    }

    #[test]
    fn store_loader_rejects_legacy_nul_values() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        fs::write(
            &path,
            r#"{"version":1,"variables":{"TOKEN":"before\u0000after"}}"#,
        )
        .unwrap();
        let error = load_store_with_test_key(&path, &TEST_ENCRYPTION_KEY).unwrap_err();
        assert!(!error.contains("before"));
    }

    #[test]
    fn invalid_import_data_does_not_change_the_saved_store() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let original = BTreeMap::from([("KEEP".to_string(), "old".to_string())]);
        save_store_with_test_key(&path, &original, &TEST_ENCRYPTION_KEY).unwrap();
        let parsed = parse_env("NEW=value\nmalformed\n");
        let batch = PendingImport {
            variables: parsed.variables,
            invalid_lines: parsed.invalid_lines,
        };
        assert!(commit_import_at_with_test_key(&path, &batch, &TEST_ENCRYPTION_KEY).is_err());
        assert_eq!(
            load_store_with_test_key(&path, &TEST_ENCRYPTION_KEY).unwrap(),
            original
        );
    }

    #[test]
    fn failed_atomic_replace_preserves_the_previous_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        fs::write(&path, "previous store").unwrap();
        let failed_replace = atomic_write_using(&path, b"new store", |_temporary, _destination| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "test failure",
            ))
        });
        assert!(failed_replace.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous store");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
