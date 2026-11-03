use crate::domain::{apply_delete, apply_save, merge_import, valid_key};
use crate::models::{PendingImport, SaveVariableInput, StoreFile, Variables};

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

pub(crate) const STORE_VERSION: u32 = 1;
pub(crate) const STORE_FILE: &str = "variables.json";

pub(crate) fn save_variable_at(path: &Path, input: SaveVariableInput) -> Result<(), String> {
    let mut variables = load_store(path)?;
    apply_save(&mut variables, input)?;
    save_store(path, &variables)
}

pub(crate) fn delete_variable_at(path: &Path, key: &str) -> Result<(), String> {
    let mut variables = load_store(path)?;
    apply_delete(&mut variables, key)?;
    save_store(path, &variables)
}

pub(crate) fn clear_variables_at(path: &Path) -> Result<(), String> {
    save_store(path, &Variables::new())
}

pub(crate) fn load_store(path: &Path) -> Result<Variables, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Variables::new()),
        Err(_) => {
            return Err(
                "Unable to read the saved variable store. Check its file permissions.".to_string(),
            )
        }
    };

    let store: StoreFile = serde_json::from_str(&content).map_err(|_| {
        "The saved variable store is invalid JSON. Move or repair the store file to continue."
            .to_string()
    })?;
    if store.version != STORE_VERSION {
        return Err("This saved variable store uses an unsupported format version.".to_string());
    }
    if store.variables.keys().any(|key| !valid_key(key)) {
        return Err(
            "The saved variable store contains an invalid key. Repair the store file to continue."
                .to_string(),
        );
    }
    if store.variables.values().any(|value| value.contains('\0')) {
        return Err("The saved variable store contains a value that cannot be used in an environment. Repair the store file to continue.".to_string());
    }
    Ok(store.variables)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    atomic_write_using(path, bytes, |temporary, destination| {
        fs::rename(temporary, destination)
    })
}

pub(crate) fn atomic_write_using(
    path: &Path,
    bytes: &[u8],
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Unable to determine the variable store directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|_| "Unable to create the variable store directory.".to_string())?;

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
        return Err(
            "Unable to write the variable store. Check available disk space and permissions."
                .to_string(),
        );
    }
    drop(file);

    if replace(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(
            "Unable to replace the variable store. Check its directory permissions.".to_string(),
        );
    }

    #[cfg(unix)]
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

pub(crate) fn save_store(path: &Path, variables: &Variables) -> Result<(), String> {
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

pub(crate) fn commit_import_at(
    path: &Path,
    batch: &PendingImport,
) -> Result<crate::models::ImportResult, String> {
    if !batch.invalid_lines.is_empty() {
        return Err(
            "This import contains invalid lines. Fix them and choose the file again.".to_string(),
        );
    }
    let existing = load_store(path)?;
    let (merged, result) = merge_import(&existing, &batch.variables);
    save_store(path, &merged)?;
    Ok(result)
}

pub(crate) fn app_store_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_config_dir().map_err(|_| {
        "Unable to locate the per-user application configuration directory.".to_string()
    })?;
    Ok(directory.join(STORE_FILE))
}

pub(crate) fn app_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app_store_file_path(app)?;
    let directory = path
        .parent()
        .ok_or_else(|| "Unable to determine the variable store directory.".to_string())?;
    fs::create_dir_all(directory).map_err(|_| {
        "Unable to create the per-user application configuration directory.".to_string()
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(|_| {
            "Unable to secure the per-user application configuration directory.".to_string()
        })?;
    }
    Ok(path)
}

pub(crate) fn with_store<T>(
    app: &AppHandle,
    operation: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let path = app_store_path(app)?;
    operation(&path)
}
