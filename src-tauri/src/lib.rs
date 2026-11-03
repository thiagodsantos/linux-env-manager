use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

const STORE_VERSION: u32 = 1;
const STORE_FILE: &str = "variables.json";
const SHELL_BLOCK_START: &[u8] = b"# >>> linux-env-manager managed shell integration >>>";
const SHELL_BLOCK_END: &[u8] = b"# <<< linux-env-manager managed shell integration <<<";

type Variables = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Bash,
    Zsh,
}

impl Shell {
    fn startup_file(self) -> &'static str {
        match self {
            Self::Bash => "~/.bashrc",
            Self::Zsh => "~/.zshrc",
        }
    }

    fn filename(self) -> &'static str {
        match self {
            Self::Bash => ".bashrc",
            Self::Zsh => ".zshrc",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellIntegration {
    pub shell: Shell,
    pub enabled: bool,
    pub startup_file: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVariableInput {
    pub key: String,
    pub value: String,
    pub original_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InvalidImportLine {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub token: String,
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub invalid_lines: Vec<InvalidImportLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub added: Vec<String>,
    pub updated: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoreFile {
    version: u32,
    variables: Variables,
}

#[derive(Default)]
struct ParsedEnv {
    variables: Variables,
    invalid_lines: Vec<InvalidImportLine>,
}

#[derive(Clone)]
struct PendingImport {
    variables: Variables,
    invalid_lines: Vec<InvalidImportLine>,
}

#[derive(Default)]
struct AppState {
    pending_imports: Mutex<HashMap<String, PendingImport>>,
    store_lock: Mutex<()>,
}

fn lock_store(state: &AppState) -> Result<MutexGuard<'_, ()>, String> {
    state.store_lock.lock()
        .map_err(|_| "Variable store is unavailable. Restart the app and try again.".to_string())
}

pub fn valid_key(key: &str) -> bool {
    let mut characters = key.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn validate_key(key: &str) -> Result<(), String> {
    if valid_key(key) {
        Ok(())
    } else {
        Err("Keys must match [A-Za-z_][A-Za-z0-9_]*.".to_string())
    }
}

fn validate_value(value: &str) -> Result<(), String> {
    if value.contains('\0') {
        Err("Variable values cannot contain NUL characters.".to_string())
    } else {
        Ok(())
    }
}

fn parse_env(contents: &str) -> ParsedEnv {
    let mut parsed = ParsedEnv::default();

    for (index, raw_line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let assignment = line.strip_prefix("export ").unwrap_or(line);
        let Some((raw_key, raw_value)) = assignment.split_once('=') else {
            parsed.invalid_lines.push(InvalidImportLine {
                line: line_number,
                message: "Expected KEY=value assignment.".to_string(),
            });
            continue;
        };

        let key = raw_key.trim();
        if !valid_key(key) {
            parsed.invalid_lines.push(InvalidImportLine {
                line: line_number,
                message: "Invalid variable key.".to_string(),
            });
            continue;
        }

        let value = raw_value.trim();
        let decoded = if value.starts_with('\'') || value.starts_with('"') {
            let quote = value.chars().next().unwrap_or_default();
            if value.len() < 2 || !value.ends_with(quote) {
                parsed.invalid_lines.push(InvalidImportLine {
                    line: line_number,
                    message: "Quoted values must have matching outer quotes.".to_string(),
                });
                continue;
            }
            value[quote.len_utf8()..value.len() - quote.len_utf8()].to_string()
        } else {
            if value.ends_with('\'') || value.ends_with('"') {
                parsed.invalid_lines.push(InvalidImportLine {
                    line: line_number,
                    message: "Quoted values must have matching outer quotes.".to_string(),
                });
                continue;
            }
            value.to_string()
        };

        if validate_value(&decoded).is_err() {
            parsed.invalid_lines.push(InvalidImportLine {
                line: line_number,
                message: "Variable values cannot contain NUL characters.".to_string(),
            });
            continue;
        }

        // Repeated assignments are deterministic: the final valid assignment wins.
        parsed.variables.insert(key.to_string(), decoded);
    }

    parsed
}

fn merge_import(existing: &Variables, incoming: &Variables) -> (Variables, ImportResult) {
    let mut merged = existing.clone();
    let mut result = ImportResult {
        added: Vec::new(),
        updated: Vec::new(),
    };

    for (key, value) in incoming {
        if merged.contains_key(key) {
            result.updated.push(key.clone());
        } else {
            result.added.push(key.clone());
        }
        merged.insert(key.clone(), value.clone());
    }

    (merged, result)
}

fn apply_save(variables: &mut Variables, input: SaveVariableInput) -> Result<(), String> {
    validate_key(&input.key)?;
    validate_value(&input.value)?;

    if let Some(original_key) = input.original_key {
        if !variables.contains_key(&original_key) {
            return Err("The variable being edited no longer exists. Reload and try again.".to_string());
        }
        if input.key != original_key && variables.contains_key(&input.key) {
            return Err("A variable with this key already exists.".to_string());
        }
        if input.key != original_key {
            variables.remove(&original_key);
        }
    } else if variables.contains_key(&input.key) {
        return Err("A variable with this key already exists.".to_string());
    }

    variables.insert(input.key, input.value);
    Ok(())
}

fn apply_delete(variables: &mut Variables, key: &str) -> Result<(), String> {
    if variables.remove(key).is_none() {
        return Err("The variable no longer exists. Reload and try again.".to_string());
    }
    Ok(())
}

fn save_variable_at(path: &Path, input: SaveVariableInput) -> Result<(), String> {
    let mut variables = load_store(path)?;
    apply_save(&mut variables, input)?;
    save_store(path, &variables)
}

fn delete_variable_at(path: &Path, key: &str) -> Result<(), String> {
    let mut variables = load_store(path)?;
    apply_delete(&mut variables, key)?;
    save_store(path, &variables)
}

fn load_store(path: &Path) -> Result<Variables, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Variables::new()),
        Err(_) => return Err("Unable to read the saved variable store. Check its file permissions.".to_string()),
    };

    let store: StoreFile = serde_json::from_str(&content)
        .map_err(|_| "The saved variable store is invalid JSON. Move or repair the store file to continue.".to_string())?;
    if store.version != STORE_VERSION {
        return Err("This saved variable store uses an unsupported format version.".to_string());
    }
    if store.variables.keys().any(|key| !valid_key(key)) {
        return Err("The saved variable store contains an invalid key. Repair the store file to continue.".to_string());
    }
    if store.variables.values().any(|value| value.contains('\0')) {
        return Err("The saved variable store contains a value that cannot be used in an environment. Repair the store file to continue.".to_string());
    }
    Ok(store.variables)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    atomic_write_using(path, bytes, |temporary, destination| fs::rename(temporary, destination))
}

fn atomic_write_using(
    path: &Path,
    bytes: &[u8],
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "Unable to determine the variable store directory.".to_string())?;
    fs::create_dir_all(parent).map_err(|_| "Unable to create the variable store directory.".to_string())?;

    let temporary = parent.join(format!(".variables-{}.tmp", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(&temporary)
        .map_err(|_| "Unable to create a temporary variable store file.".to_string())?;
    let write_result = file.write_all(bytes).and_then(|()| file.sync_all());
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("Unable to write the variable store. Check available disk space and permissions.".to_string());
    }
    drop(file);

    if replace(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("Unable to replace the variable store. Check its directory permissions.".to_string());
    }

    #[cfg(unix)]
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

fn save_store(path: &Path, variables: &Variables) -> Result<(), String> {
    if variables.keys().any(|key| !valid_key(key)) {
        return Err("Keys must match [A-Za-z_][A-Za-z0-9_]*.".to_string());
    }
    if variables.values().any(|value| value.contains('\0')) {
        return Err("Variable values cannot contain NUL characters.".to_string());
    }
    let payload = StoreFile {
        version: STORE_VERSION,
        variables: variables.clone(),
    };
    let encoded = serde_json::to_vec_pretty(&payload)
        .map_err(|_| "Unable to encode the variable store.".to_string())?;
    atomic_write(path, &encoded)
}

fn commit_import_at(path: &Path, batch: &PendingImport) -> Result<ImportResult, String> {
    if !batch.invalid_lines.is_empty() {
        return Err("This import contains invalid lines. Fix them and choose the file again.".to_string());
    }
    let existing = load_store(path)?;
    let (merged, result) = merge_import(&existing, &batch.variables);
    save_store(path, &merged)?;
    Ok(result)
}

fn app_store_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_config_dir()
        .map_err(|_| "Unable to locate the per-user application configuration directory.".to_string())?;
    Ok(directory.join(STORE_FILE))
}

fn app_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app_store_file_path(app)?;
    let directory = path.parent()
        .ok_or_else(|| "Unable to determine the variable store directory.".to_string())?;
    fs::create_dir_all(directory)
        .map_err(|_| "Unable to create the per-user application configuration directory.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Unable to secure the per-user application configuration directory.".to_string())?;
    }
    Ok(path)
}

fn with_store<T>(app: &AppHandle, operation: impl FnOnce(&Path) -> Result<T, String>) -> Result<T, String> {
    let path = app_store_path(app)?;
    operation(&path)
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || "_@%+=:,./-".contains(character)
        })
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn managed_shell_block(store_path: &Path) -> Result<String, String> {
    const PYTHON_EXPORT_SCRIPT: &str = r#"import json
import re
import shlex
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as store_file:
        store = json.load(store_file)
    if isinstance(store, dict) and store.get("version") == 1:
        variables = store.get("variables", {})
        if isinstance(variables, dict):
            for key, value in variables.items():
                if isinstance(key, str) and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key) and isinstance(value, str) and "\x00" not in value:
                    print("export " + key + "=" + shlex.quote(value))
except (OSError, ValueError, TypeError, AttributeError):
    pass
"#;

    let store_path = store_path.to_str()
        .ok_or_else(|| "The application variable store path is not valid UTF-8.".to_string())?;
    Ok(format!(
        "{}\nif command -v python3 >/dev/null 2>&1; then\n  eval \"$(python3 -c {} {})\"\nfi\n{}\n",
        String::from_utf8_lossy(SHELL_BLOCK_START),
        shell_quote(PYTHON_EXPORT_SCRIPT),
        shell_quote(store_path),
        String::from_utf8_lossy(SHELL_BLOCK_END),
    ))
}

fn managed_block_range(contents: &[u8]) -> Result<Option<Range<usize>>, String> {
    let mut start = None;
    let mut end = None;
    let mut offset = 0;

    for line in contents.split_inclusive(|byte| *byte == b'\n') {
        let mut marker = line.strip_suffix(b"\n").unwrap_or(line);
        marker = marker.strip_suffix(b"\r").unwrap_or(marker);
        if marker == SHELL_BLOCK_START {
            if start.replace(offset).is_some() {
                return Err("The shell startup file contains duplicate managed integration markers.".to_string());
            }
        } else if marker == SHELL_BLOCK_END {
            if end.replace(offset + line.len()).is_some() {
                return Err("The shell startup file contains duplicate managed integration markers.".to_string());
            }
        }
        offset += line.len();
    }

    match (start, end) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) if start < end => Ok(Some(start..end)),
        _ => Err("The shell startup file contains an incomplete or misplaced managed integration block.".to_string()),
    }
}

fn replace_managed_block(contents: &[u8], replacement: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let Some(range) = managed_block_range(contents)? else {
        let Some(replacement) = replacement else {
            return Ok(contents.to_vec());
        };
        let mut updated = contents.to_vec();
        if !updated.is_empty() && !updated.ends_with(b"\n") {
            updated.push(b'\n');
        }
        updated.extend_from_slice(replacement);
        return Ok(updated);
    };

    let mut updated = Vec::with_capacity(
        contents.len() - (range.end - range.start) + replacement.map_or(0, |bytes| bytes.len()),
    );
    updated.extend_from_slice(&contents[..range.start]);
    if let Some(replacement) = replacement {
        updated.extend_from_slice(replacement);
    }
    updated.extend_from_slice(&contents[range.end..]);
    Ok(updated)
}

fn shell_startup_path(app: &AppHandle, shell: Shell) -> Result<PathBuf, String> {
    let home = app.path().home_dir()
        .map_err(|_| "Unable to locate the current user's home directory.".to_string())?;
    Ok(home.join(shell.filename()))
}

fn shell_integration_enabled(path: &Path) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("Unable to inspect the shell startup file.".to_string()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("The shell startup file must be a regular, non-symbolic-link file.".to_string());
    }
    let contents = fs::read(path)
        .map_err(|_| "Unable to read the shell startup file.".to_string())?;
    Ok(managed_block_range(&contents)?.is_some())
}

fn atomic_write_startup_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("The shell startup file must be a regular, non-symbolic-link file.".to_string());
            }
            Some(metadata)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("Unable to inspect the shell startup file.".to_string()),
    };
    let parent = path.parent()
        .ok_or_else(|| "Unable to determine the shell startup directory.".to_string())?;
    let temporary = parent.join(format!(".linux-env-manager-{}.tmp", Uuid::new_v4()));

    let mode = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.as_ref().map(|metadata| metadata.permissions().mode() & 0o777).unwrap_or(0o600)
        }
        #[cfg(not(unix))]
        {
            0
        }
    };
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    let mut file = options.open(&temporary)
        .map_err(|_| "Unable to create a temporary shell startup file.".to_string())?;
    if file.write_all(bytes).and_then(|()| file.sync_all()).is_err() {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err("Unable to write the shell startup file.".to_string());
    }
    drop(file);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::set_permissions(&temporary, fs::Permissions::from_mode(mode)).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("Unable to preserve shell startup file permissions.".to_string());
        }
    }
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("Unable to replace the shell startup file.".to_string());
    }
    #[cfg(unix)]
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn set_shell_integration_at(
    startup_path: &Path,
    store_path: Option<&Path>,
    enabled: bool,
) -> Result<(), String> {
    let block = if enabled {
        let store_path = store_path.ok_or_else(|| "Unable to locate the variable store.".to_string())?;
        Some(managed_shell_block(store_path)?.into_bytes())
    } else {
        None
    };
    let existing = match fs::symlink_metadata(startup_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("The shell startup file must be a regular, non-symbolic-link file.".to_string());
            }
            fs::read(startup_path)
                .map_err(|_| "Unable to read the shell startup file.".to_string())?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return Err("Unable to inspect the shell startup file.".to_string()),
    };
    let updated = replace_managed_block(&existing, block.as_deref())?;
    if updated != existing {
        atomic_write_startup_file(startup_path, &updated)?;
    }
    Ok(())
}

#[tauri::command]
fn get_shell_integrations(app: AppHandle) -> Result<Vec<ShellIntegration>, String> {
    [Shell::Bash, Shell::Zsh]
        .into_iter()
        .map(|shell| {
            let path = shell_startup_path(&app, shell)?;
            Ok(ShellIntegration {
                shell,
                enabled: shell_integration_enabled(&path)?,
                startup_file: shell.startup_file(),
            })
        })
        .collect()
}

#[tauri::command]
fn set_shell_integration(
    app: AppHandle,
    state: State<'_, AppState>,
    shell: Shell,
    enabled: bool,
) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    if enabled && !python3_available() {
        return Err("Python 3 is required for shell integration. Install python3 and retry.".to_string());
    }
    let startup_path = shell_startup_path(&app, shell)?;
    if enabled {
        let store_path = app_store_file_path(&app)?;
        set_shell_integration_at(&startup_path, Some(&store_path), true)
    } else {
        set_shell_integration_at(&startup_path, None, false)
    }
}

#[tauri::command]
fn list_variables(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<Variable>, String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| {
        Ok(load_store(path)?.into_iter().map(|(key, value)| Variable { key, value }).collect())
    })
}

#[tauri::command]
fn save_variable(app: AppHandle, state: State<'_, AppState>, input: SaveVariableInput) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| save_variable_at(path, input))
}

#[tauri::command]
fn delete_variable(app: AppHandle, state: State<'_, AppState>, key: String) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| delete_variable_at(path, &key))
}

#[tauri::command]
async fn preview_import(app: AppHandle, state: State<'_, AppState>) -> Result<Option<ImportPreview>, String> {
    let Some(file) = app.dialog().file().add_filter("Environment files", &["env"]).blocking_pick_file() else {
        return Ok(None);
    };
    let file_path = file.into_path().map_err(|_| "Unable to access the selected environment file.".to_string())?;
    let content = fs::read_to_string(file_path)
        .map_err(|_| "Unable to read the selected file as UTF-8 text.".to_string())?;
    let parsed = parse_env(&content);
    let store_path = app_store_path(&app)?;
    let existing = {
        let _guard = lock_store(&state)?;
        load_store(&store_path)?
    };
    let (_, changes) = merge_import(&existing, &parsed.variables);
    let token = Uuid::new_v4().to_string();
    let preview = ImportPreview {
        token: token.clone(),
        added: changes.added,
        updated: changes.updated,
        invalid_lines: parsed.invalid_lines.clone(),
    };
    state.pending_imports.lock()
        .map_err(|_| "Import preview state is unavailable. Restart the app and try again.".to_string())?
        .insert(token, PendingImport { variables: parsed.variables, invalid_lines: parsed.invalid_lines });
    Ok(Some(preview))
}

#[tauri::command]
fn discard_import(state: State<'_, AppState>, token: String) -> Result<(), String> {
    state.pending_imports.lock()
        .map_err(|_| "Import preview state is unavailable. Restart the app and try again.".to_string())?
        .remove(&token);
    Ok(())
}

#[tauri::command]
fn apply_import(app: AppHandle, state: State<'_, AppState>, token: String) -> Result<ImportResult, String> {
    let _store_guard = lock_store(&state)?;
    let path = app_store_path(&app)?;
    let mut pending = state.pending_imports.lock()
        .map_err(|_| "Import preview state is unavailable. Restart the app and try again.".to_string())?;
    let batch = pending.get(&token).cloned().ok_or_else(|| "This import preview expired. Choose the file again.".to_string())?;
    let result = commit_import_at(&path, &batch)?;
    pending.remove(&token);
    Ok(result)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_variables,
            save_variable,
            delete_variable,
            preview_import,
            apply_import,
            discard_import,
            get_shell_integrations,
            set_shell_integration
        ])
        .run(tauri::generate_context!())
        .expect("error while running environment manager");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn validates_environment_variable_names() {
        for key in ["A", "_private", "PATH", "HOME_2"] {
            assert!(valid_key(key), "{key} should be valid");
        }
        for key in ["", "2FAST", "HAS-DASH", "WITH SPACE", "é" ] {
            assert!(!valid_key(key), "{key} should be invalid");
        }
    }

    #[test]
    fn rejects_nul_values_in_crud_and_import() {
        let mut variables = BTreeMap::new();
        let saved = apply_save(&mut variables, SaveVariableInput {
            key: "TOKEN".to_string(),
            value: "before\0after".to_string(),
            original_key: None,
        });
        assert!(saved.is_err());
        assert!(variables.is_empty());

        let parsed = parse_env("TOKEN=before\0after\nGOOD=value\n");
        assert_eq!(parsed.invalid_lines.len(), 1);
        assert_eq!(parsed.variables.get("GOOD").map(String::as_str), Some("value"));
        assert!(!parsed.invalid_lines[0].message.contains("before"));
    }

    #[test]
    fn shell_quoting_handles_spaces_quotes_and_shell_syntax() {
        assert_eq!(shell_quote("plain/path"), "plain/path");
        assert_eq!(shell_quote("a'b $(touch unsafe)"), "'a'\\''b $(touch unsafe)'");
    }

    #[test]
    fn generated_shell_block_reads_the_store_and_quotes_its_path() {
        let block = managed_shell_block(Path::new("/tmp/app's data/variables.json")).unwrap();
        assert!(block.starts_with("# >>> linux-env-manager managed shell integration >>>\n"));
        assert!(block.contains("python3 -c"));
        assert!(block.contains("json.load(store_file)"));
        assert!(block.contains("shlex.quote(value)"));
        assert!(block.contains("/tmp/app'\\''s data/variables.json"));
        assert!(block.ends_with("# <<< linux-env-manager managed shell integration <<<\n"));
    }

    #[test]
    fn shell_integration_contract_uses_the_required_names() {
        let integrations = vec![
            ShellIntegration { shell: Shell::Bash, enabled: true, startup_file: Shell::Bash.startup_file() },
            ShellIntegration { shell: Shell::Zsh, enabled: false, startup_file: Shell::Zsh.startup_file() },
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
        let parsed = parse_env(
            "# comment\n\nA=one=two\nexport B='quoted value'\nC=\"$A\"\nEMPTY=\"\"\n",
        );
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
        assert!(!parsed.invalid_lines.iter().any(|line| line.message.contains("secret-value")));
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
        assert!(apply_save(&mut variables, SaveVariableInput {
            key: "SECOND".to_string(), value: "overwrite".to_string(), original_key: None,
        }).is_err());
        apply_save(&mut variables, SaveVariableInput {
            key: "FIRST".to_string(), value: "updated".to_string(), original_key: Some("FIRST".to_string()),
        }).unwrap();
        assert_eq!(variables["FIRST"], "updated");
        assert!(apply_save(&mut variables, SaveVariableInput {
            key: "SECOND".to_string(), value: "collision".to_string(), original_key: Some("FIRST".to_string()),
        }).is_err());
        apply_save(&mut variables, SaveVariableInput {
            key: "RENAMED".to_string(), value: "renamed".to_string(), original_key: Some("FIRST".to_string()),
        }).unwrap();
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
        save_store(&path, &original).unwrap();
        let parsed = parse_env("EXISTING=new\nNEW=value\n");
        let batch = PendingImport { variables: parsed.variables, invalid_lines: parsed.invalid_lines };
        let result = commit_import_at(&path, &batch).unwrap();
        assert_eq!(result.added, vec!["NEW"]);
        assert_eq!(result.updated, vec!["EXISTING"]);
        let persisted = load_store(&path).unwrap();
        assert_eq!(persisted["EXISTING"], "new");
        assert_eq!(persisted["NEW"], "value");
    }

    #[test]
    fn store_round_trips_and_uses_restrictive_permissions() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let variables = BTreeMap::from([("TOKEN".to_string(), "secret".to_string())]);
        save_store(&path, &variables).unwrap();
        assert_eq!(load_store(&path).unwrap(), variables);
        assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn store_loader_rejects_legacy_nul_values() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        fs::write(&path, r#"{"version":1,"variables":{"TOKEN":"before\u0000after"}}"#).unwrap();
        let error = load_store(&path).unwrap_err();
        assert!(!error.contains("before"));
    }

    #[test]
    fn invalid_import_data_does_not_change_the_saved_store() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        let original = BTreeMap::from([("KEEP".to_string(), "old".to_string())]);
        save_store(&path, &original).unwrap();
        let parsed = parse_env("NEW=value\nmalformed\n");
        let batch = PendingImport { variables: parsed.variables, invalid_lines: parsed.invalid_lines };
        assert!(commit_import_at(&path, &batch).is_err());
        assert_eq!(load_store(&path).unwrap(), original);
    }

    #[test]
    fn failed_atomic_replace_preserves_the_previous_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(STORE_FILE);
        fs::write(&path, "previous store").unwrap();
        let failed_replace = atomic_write_using(&path, b"new store", |_temporary, _destination| {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test failure"))
        });
        assert!(failed_replace.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous store");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
