use crate::domain::{
    apply_delete, create_profile, effective_variables, merge_import, save_profile_variable,
    valid_color, valid_key,
};
use crate::models::{
    CreateProfileInput, PendingImport, Profile, ProfileData, Profiles, SaveVariableInput,
    StoreFile, Variable, Variables,
};
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

pub(crate) const STORE_VERSION: u32 = 2;
const LEGACY_STORE_VERSION: u32 = 1;
pub(crate) const ENCRYPTED_STORE_VERSION: u32 = 2;
pub(crate) const STORE_FILE: &str = "variables.json";
const ENCRYPTION_ALGORITHM: &str = "AES-256-GCM";
const KEYRING_SERVICE: &str = "dev.linuxenvmanager.app";
const KEYRING_ACCOUNT: &str = "variables-encryption-key-v1";
const GENERAL_PROFILE: &str = "General";

#[derive(Serialize, Deserialize)]
struct EncryptedStore {
    version: u32,
    algorithm: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Deserialize)]
struct LegacyStoreFile {
    version: u32,
    variables: Variables,
}

fn default_store() -> StoreFile {
    StoreFile {
        version: STORE_VERSION,
        profiles: Profiles::from([(GENERAL_PROFILE.to_string(), ProfileData::general())]),
        active_profile: GENERAL_PROFILE.to_string(),
    }
}

pub(crate) fn list_profiles_at(path: &Path) -> Result<Vec<Profile>, String> {
    let store = load_store(path)?;
    Ok(store
        .profiles
        .iter()
        .map(|(name, profile)| profile.public(name, store.active_profile == *name))
        .collect())
}

pub(crate) fn list_variables_at(path: &Path, profile_name: &str) -> Result<Vec<Variable>, String> {
    let store = load_store(path)?;
    let effective = effective_variables(&store.profiles, profile_name)?;
    let selected = store.profiles.get(profile_name).ok_or_else(|| {
        "The selected profile no longer exists. Reload and try again.".to_string()
    })?;
    Ok(effective
        .into_iter()
        .map(|(key, value)| Variable {
            inherited: profile_name != GENERAL_PROFILE && !selected.variables.contains_key(&key),
            key,
            value,
        })
        .collect())
}

pub(crate) fn create_profile_at(path: &Path, input: CreateProfileInput) -> Result<(), String> {
    let name = input.name.trim().to_string();
    let import_from = input.import_from.clone();
    let mut store = load_store(path)?;
    let imported = import_from
        .as_deref()
        .map(|source_name| effective_variables(&store.profiles, source_name))
        .transpose()?;
    create_profile(&mut store.profiles, input)?;
    if let Some(variables) = imported {
        store
            .profiles
            .get_mut(&name)
            .ok_or_else(|| "Unable to create the requested profile.".to_string())?
            .variables = variables;
    }
    store.active_profile = name;
    save_store(path, &store)
}

pub(crate) fn delete_profile_at(path: &Path, profile_name: &str) -> Result<(), String> {
    let mut store = load_store(path)?;
    remove_profile(&mut store, profile_name)?;
    save_store(path, &store)
}

pub(crate) fn remove_profile(store: &mut StoreFile, profile_name: &str) -> Result<(), String> {
    if profile_name == GENERAL_PROFILE {
        return Err("The General profile cannot be deleted.".to_string());
    }
    if store.profiles.remove(profile_name).is_none() {
        return Err("The profile no longer exists. Reload and try again.".to_string());
    }
    if store.active_profile == profile_name {
        store.active_profile = GENERAL_PROFILE.to_string();
    }
    Ok(())
}

pub(crate) fn set_active_profile_at(path: &Path, profile_name: &str) -> Result<(), String> {
    let mut store = load_store(path)?;
    if !store.profiles.contains_key(profile_name) {
        return Err("The selected profile no longer exists. Reload and try again.".to_string());
    }
    store.active_profile = profile_name.to_string();
    save_store(path, &store)
}

pub(crate) fn save_variable_at(path: &Path, input: SaveVariableInput) -> Result<(), String> {
    let mut store = load_store(path)?;
    save_profile_variable(&mut store.profiles, input)?;
    save_store(path, &store)
}

pub(crate) fn delete_variable_at(path: &Path, profile_name: &str, key: &str) -> Result<(), String> {
    let mut store = load_store(path)?;
    let profile = store.profiles.get_mut(profile_name).ok_or_else(|| {
        "The selected profile no longer exists. Reload and try again.".to_string()
    })?;
    apply_delete(&mut profile.variables, key)?;
    save_store(path, &store)
}

pub(crate) fn clear_variables_at(path: &Path, profile_name: &str) -> Result<(), String> {
    let mut store = load_store(path)?;
    let profile = store.profiles.get_mut(profile_name).ok_or_else(|| {
        "The selected profile no longer exists. Reload and try again.".to_string()
    })?;
    profile.variables.clear();
    save_store(path, &store)
}

fn read_store_file(path: &Path) -> Result<Vec<u8>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => {
            Err("Unable to read the saved variable store. Check its file permissions.".to_string())
        }
    }
}

pub(crate) fn load_store(path: &Path) -> Result<StoreFile, String> {
    let bytes = read_store_file(path)?;
    if bytes.is_empty() {
        return Ok(default_store());
    }
    let key = match store_version(&bytes)? {
        ENCRYPTED_STORE_VERSION => get_encryption_key(false)?,
        LEGACY_STORE_VERSION => get_encryption_key(true)?,
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

fn load_store_from_bytes(bytes: &[u8], key: &[u8; 32], path: &Path) -> Result<StoreFile, String> {
    let outer_version = store_version(bytes)?;
    let (plaintext, should_migrate) = if outer_version == ENCRYPTED_STORE_VERSION {
        let encrypted: EncryptedStore = serde_json::from_slice(bytes).map_err(|_| {
            "The encrypted variable store is invalid. Repair or restore the store file to continue."
                .to_string()
        })?;
        if encrypted.algorithm != ENCRYPTION_ALGORITHM {
            return Err("The variable store uses an unsupported encryption algorithm.".to_string());
        }
        let nonce = BASE64.decode(encrypted.nonce).map_err(|_| {
            "The encrypted variable store is invalid. Repair or restore the store file to continue."
                .to_string()
        })?;
        if nonce.len() != 12 {
            return Err("The encrypted variable store is invalid. Repair or restore the store file to continue.".to_string());
        }
        let nonce = Nonce::from_slice(&nonce);
        let ciphertext = BASE64.decode(encrypted.ciphertext).map_err(|_| {
            "The encrypted variable store is invalid. Repair or restore the store file to continue."
                .to_string()
        })?;
        let plaintext = Aes256Gcm::new_from_slice(key)
            .map_err(|_| "Unable to initialize variable store encryption.".to_string())?
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| "Unable to decrypt the variable store. Verify that the desktop keyring is available and the store has not been changed.".to_string())?;
        (plaintext, false)
    } else if outer_version == LEGACY_STORE_VERSION {
        (bytes.to_vec(), true)
    } else {
        return Err("This saved variable store uses an unsupported format version.".to_string());
    };

    let payload_version = store_version(&plaintext)?;
    let (store, migrate_payload) = match payload_version {
        LEGACY_STORE_VERSION => {
            let legacy: LegacyStoreFile = serde_json::from_slice(&plaintext).map_err(|_| {
                "The saved variable store is invalid. Move or repair the store file to continue."
                    .to_string()
            })?;
            if legacy.version != LEGACY_STORE_VERSION {
                return Err(
                    "This saved variable store uses an unsupported format version.".to_string(),
                );
            }
            let mut migrated = default_store();
            migrated
                .profiles
                .get_mut(GENERAL_PROFILE)
                .expect("default store contains General")
                .variables = legacy.variables;
            (migrated, true)
        }
        STORE_VERSION => {
            let store: StoreFile = serde_json::from_slice(&plaintext).map_err(|_| {
                "The saved variable store is invalid. Move or repair the store file to continue."
                    .to_string()
            })?;
            (store, false)
        }
        _ => {
            return Err("This saved variable store uses an unsupported format version.".to_string())
        }
    };
    validate_store(&store)?;
    if should_migrate || migrate_payload {
        save_store_with_key(path, &store, key)?;
    }
    Ok(store)
}

fn validate_store(store: &StoreFile) -> Result<(), String> {
    if store.version != STORE_VERSION
        || !store.profiles.contains_key(GENERAL_PROFILE)
        || !store.profiles.contains_key(&store.active_profile)
    {
        return Err(
            "This saved variable store uses an unsupported or incomplete format.".to_string(),
        );
    }
    for (name, profile) in &store.profiles {
        if name != GENERAL_PROFILE {
            crate::domain::validate_profile_name(name)?;
        }
        if !valid_color(&profile.color) {
            return Err("The saved variable store contains an invalid profile color.".to_string());
        }
        if profile.description.len() > 240 {
            return Err(
                "The saved variable store contains an invalid profile description.".to_string(),
            );
        }
        if profile.variables.keys().any(|key| !valid_key(key)) {
            return Err("The saved variable store contains an invalid key. Repair the store file to continue.".to_string());
        }
        if profile.variables.values().any(|value| value.contains('\0')) {
            return Err("The saved variable store contains a value that cannot be used in an environment. Repair the store file to continue.".to_string());
        }
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
            OsRng.try_fill_bytes(&mut key)
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

pub(crate) fn save_store(path: &Path, store: &StoreFile) -> Result<(), String> {
    validate_store(store)?;
    let key = get_encryption_key(true)?;
    save_store_with_key(path, store, &key)
}

fn save_store_with_key(path: &Path, store: &StoreFile, key: &[u8; 32]) -> Result<(), String> {
    validate_store(store)?;
    let plaintext = serde_json::to_vec(store)
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
    let mut store = default_store();
    store
        .profiles
        .get_mut(GENERAL_PROFILE)
        .expect("General exists")
        .variables = variables.clone();
    save_store_with_key(path, &store, key)
}

#[cfg(test)]
pub(crate) fn clear_variables_with_test_key(path: &Path, key: &[u8; 32]) -> Result<(), String> {
    let store = default_store();
    save_store_with_key(path, &store, key)
}

#[cfg(test)]
pub(crate) fn load_store_with_test_key(path: &Path, key: &[u8; 32]) -> Result<Variables, String> {
    let bytes = read_store_file(path)?;
    if bytes.is_empty() {
        return Ok(Variables::new());
    }
    Ok(load_store_from_bytes(&bytes, key, path)?
        .profiles
        .remove(GENERAL_PROFILE)
        .unwrap_or_else(ProfileData::general)
        .variables)
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
    let mut store = load_store(path)?;
    let effective = effective_variables(&store.profiles, &batch.profile_name)?;
    let (_, result) = merge_import(&effective, &batch.variables);
    let profile = store.profiles.get_mut(&batch.profile_name).ok_or_else(|| {
        "The selected profile no longer exists. Reload and try again.".to_string()
    })?;
    let (merged, _) = merge_import(&profile.variables, &batch.variables);
    profile.variables = merged;
    save_store(path, &store)?;
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
    let bytes = read_store_file(path)?;
    let mut store = if bytes.is_empty() {
        default_store()
    } else {
        load_store_from_bytes(&bytes, key, path)?
    };
    let effective = effective_variables(&store.profiles, &batch.profile_name)?;
    let (_, result) = merge_import(&effective, &batch.variables);
    let profile = store.profiles.get_mut(&batch.profile_name).ok_or_else(|| {
        "The selected profile no longer exists. Reload and try again.".to_string()
    })?;
    let (merged, _) = merge_import(&profile.variables, &batch.variables);
    profile.variables = merged;
    save_store_with_key(path, &store, key)?;
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
