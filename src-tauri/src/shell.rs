use crate::models::{Shell, ShellIntegration};
use crate::state::{lock_store, AppState};
use crate::storage::app_store_file_path;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

pub(crate) const SHELL_BLOCK_START: &[u8] = b"# >>> linux-env-manager managed shell integration >>>";
pub(crate) const SHELL_BLOCK_END: &[u8] = b"# <<< linux-env-manager managed shell integration <<<";


pub(crate) fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || "_@%+=:,./-".contains(character)
        })
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn managed_shell_block(store_path: &Path) -> Result<String, String> {
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

pub(crate) fn managed_block_range(contents: &[u8]) -> Result<Option<Range<usize>>, String> {
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

pub(crate) fn replace_managed_block(contents: &[u8], replacement: Option<&[u8]>) -> Result<Vec<u8>, String> {
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

pub(crate) fn set_shell_integration_at(
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

pub(crate) fn get_shell_integrations(app: AppHandle) -> Result<Vec<ShellIntegration>, String> {
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

pub(crate) fn set_shell_integration(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
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
