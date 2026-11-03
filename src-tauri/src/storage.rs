use crate::domain::{apply_delete, apply_save, merge_import, valid_key};
use crate::models::{PendingImport, SaveVariableInput, StoreFile, Variables};
use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

pub(crate) const STORE_VERSION: u32 = 1;
pub(crate) const ENCRYPTED_STORE_VERSION: u32 = 2;
pub(crate) const STORE_FILE: &str = "variables.json";
const ENCRYPTION_ALGORITHM: &str = "AES-256-GCM";
const KEYRING_SERVICE: &str = "dev.linuxenvmanager.app";
const KEYRING_ACCOUNT: &str = "variables-encryption-key-v1";

#[derive(Serialize, Deserialize)]
struct EncryptedStore {
    version: u32,
    algorithm: String,
    nonce: String,
    ciphertext: String,
}

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

#[cfg(test)]
pub(crate) fn clear_variables_with_test_key(path: &Path, key: &[u8; 32]) -> Result<(), String> {
    save_store_with_key(path, &Variables::new(), key)
}

pub(crate) fn load_store(path: &Path) -> Result<Variables, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Variables::new()),
        Err(_) => {
            return Err(
                "Unable to read the saved variable store. Check its file permissions.".to_string(),
            )
        }
    };
    let version = store_version(&bytes)?;
    let key = match version {
        ENCRYPTED_STORE_VERSION => get_encryption_key(false)?,
        STORE_VERSION => get_encryption_key(true)?,
        _ => {
            return Err("This saved variable store uses an unsupported format version.".to_string())
        }
    };
    load_store_from_bytes(&bytes, &key, path)
}

fn store_version(bytes: &[u8]) -> Result<u32, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| {
        "The saved variable store is invalid JSON. Move or repair the store file to continue."
            .to_string()
    })?;
    value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .ok_or_else(|| {
            "The saved variable store is invalid. Move or repair the store file to continue."
                .to_string()
        })
}

fn load_store_from_bytes(bytes: &[u8], key: &[u8; 32], path: &Path) -> Result<Variables, String> {
    let version = store_version(bytes)?;
    let plaintext = if version == ENCRYPTED_STORE_VERSION {
        let encrypted: EncryptedStore = serde_json::from_slice(bytes)
            .map_err(|_| "The encrypted variable store is invalid. Repair or restore the store file to continue.".to_string())?;
        if encrypted.algorithm != ENCRYPTION_ALGORITHM {
            return Err("The variable store uses an unsupported encryption algorithm.".to_string());
        }
        let nonce = BASE64.decode(encrypted.nonce)
            .map_err(|_| "The encrypted variable store is invalid. Repair or restore the store file to continue.".to_string())?;
        if nonce.len() != 12 {
            return Err("The encrypted variable store is invalid. Repair or restore the store file to continue.".to_string());
        }
        let nonce = Nonce::from_slice(&nonce);
        let ciphertext = BASE64.decode(encrypted.ciphertext)
            .map_err(|_| "The encrypted variable store is invalid. Repair or restore the store file to continue.".to_string())?;
        Aes256Gcm::new_from_slice(key)
            .map_err(|_| "Unable to initialize variable store encryption.".to_string())?
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| "Unable to decrypt the variable store. Verify that the desktop keyring is available and the store has not been changed.".to_string())?
    } else if version == STORE_VERSION {
        bytes.to_vec()
    } else {
        return Err("This saved variable store uses an unsupported format version.".to_string());
    };

    let store: StoreFile = serde_json::from_slice(&plaintext).map_err(|_| {
        "The saved variable store is invalid. Move or repair the store file to continue."
            .to_string()
    })?;
    validate_store(&store)?;
    if version == STORE_VERSION {
        save_store_with_key(path, &store.variables, key)?;
    }
    Ok(store.variables)
}

fn validate_store(store: &StoreFile) -> Result<(), String> {
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
    Ok(())
}

fn get_encryption_key(create_if_missing: bool) -> Result<[u8; 32], String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|_| "Unable to access the desktop keyring. Unlock or configure a Secret Service keyring and try again.".to_string())?;
    match entry.get_password() {
        Ok(encoded) => {
            let key = BASE64.decode(encoded)
                .map_err(|_| "The desktop keyring encryption key is invalid. Do not delete the variable store; restore the keyring entry or a backup.".to_string())?;
            key.try_into()
                .map_err(|_| "The desktop keyring encryption key is invalid. Do not delete the variable store; restore the keyring entry or a backup.".to_string())
        }
        Err(keyring::Error::NoEntry) if create_if_missing => {
            let mut key = [0u8; 32];
            OsRng
                .try_fill_bytes(&mut key)
                .map_err(|_| "Unable to generate a secure encryption key.".to_string())?;
            entry.set_password(&BASE64.encode(key))
                .map_err(|_| "Unable to save an encryption key in the desktop keyring. Unlock or configure a Secret Service keyring and try again.".to_string())?;
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => Err("The encryption key for this variable store is missing from the desktop keyring. Do not delete the store; restore the keyring entry or a backup.".to_string()),
        Err(_) => Err("Unable to access the desktop keyring. Unlock or configure a Secret Service keyring and try again.".to_string()),
    }
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
    validate_variables(variables)?;
    let key = get_encryption_key(true)?;
    save_store_with_key(path, variables, &key)
}

fn validate_variables(variables: &Variables) -> Result<(), String> {
    if variables.keys().any(|key| !valid_key(key)) {
        return Err("Keys must match [A-Za-z_][A-Za-z0-9_]*.".to_string());
    }
    if variables.values().any(|value| value.contains('\0')) {
        return Err("Variable values cannot contain NUL characters.".to_string());
    }
    Ok(())
}

fn save_store_with_key(path: &Path, variables: &Variables, key: &[u8; 32]) -> Result<(), String> {
    validate_variables(variables)?;
    let payload = StoreFile {
        version: STORE_VERSION,
        variables: variables.clone(),
    };
    let plaintext = serde_json::to_vec(&payload)
        .map_err(|_| "Unable to encode the variable store.".to_string())?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| "Unable to initialize variable store encryption.".to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|_| "Unable to generate a secure encryption nonce.".to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|_| "Unable to encrypt the variable store.".to_string())?;
    let encrypted = EncryptedStore {
        version: ENCRYPTED_STORE_VERSION,
        algorithm: ENCRYPTION_ALGORITHM.to_string(),
        nonce: BASE64.encode(nonce_bytes),
        ciphertext: BASE64.encode(ciphertext),
    };
    let encoded = serde_json::to_vec_pretty(&encrypted)
        .map_err(|_| "Unable to encode the encrypted variable store.".to_string())?;
    atomic_write(path, &encoded)
}

#[cfg(test)]
pub(crate) fn save_store_with_test_key(
    path: &Path,
    variables: &Variables,
    key: &[u8; 32],
) -> Result<(), String> {
    save_store_with_key(path, variables, key)
}

#[cfg(test)]
pub(crate) fn load_store_with_test_key(path: &Path, key: &[u8; 32]) -> Result<Variables, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Variables::new()),
        Err(_) => {
            return Err(
                "Unable to read the saved variable store. Check its file permissions.".to_string(),
            )
        }
    };
    load_store_from_bytes(&bytes, key, path)
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

#[cfg(test)]
pub(crate) fn commit_import_at_with_test_key(
    path: &Path,
    batch: &PendingImport,
    key: &[u8; 32],
) -> Result<crate::models::ImportResult, String> {
    if !batch.invalid_lines.is_empty() {
        return Err(
            "This import contains invalid lines. Fix them and choose the file again.".to_string(),
        );
    }
    let existing = load_store_with_test_key(path, key)?;
    let (merged, result) = merge_import(&existing, &batch.variables);
    save_store_with_key(path, &merged, key)?;
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
