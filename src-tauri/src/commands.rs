use crate::domain::{merge_import, parse_env};
use crate::models::{
    CreateProfileInput, ImportPreview, ImportResult, PendingImport, SaveVariableInput, Shell,
};
use crate::shell::{
    get_shell_integrations as shell_integrations, set_shell_integration as update_shell_integration,
};
use crate::state::{lock_store, AppState};
use crate::storage::{
    app_store_path, clear_variables_at, commit_import_at, create_profile_at, delete_profile_at,
    delete_variable_at, list_profiles_at, list_variables_at, load_store, save_variable_at,
    set_active_profile_at, with_store,
};
use std::fs;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

#[tauri::command]
fn get_shell_integrations(app: AppHandle) -> Result<Vec<crate::models::ShellIntegration>, String> {
    shell_integrations(app)
}

#[tauri::command]
fn set_shell_integration(
    app: AppHandle,
    state: State<'_, AppState>,
    shell: Shell,
    enabled: bool,
) -> Result<(), String> {
    update_shell_integration(app, state, shell, enabled)
}

#[tauri::command]
fn list_profiles(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::Profile>, String> {
    let _guard = lock_store(&state)?;
    with_store(&app, list_profiles_at)
}

#[tauri::command]
fn create_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateProfileInput,
) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| create_profile_at(path, input))
}

#[tauri::command]
fn set_active_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| set_active_profile_at(path, &profile_name))
}

#[tauri::command]
fn delete_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| delete_profile_at(path, &profile_name))
}

#[tauri::command]
fn list_variables(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<Vec<crate::models::Variable>, String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| list_variables_at(path, &profile_name))
}

#[tauri::command]
fn save_variable(
    app: AppHandle,
    state: State<'_, AppState>,
    input: SaveVariableInput,
) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| save_variable_at(path, input))
}

#[tauri::command]
fn delete_variable(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
    key: String,
) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| delete_variable_at(path, &profile_name, &key))
}

#[tauri::command]
fn clear_variables(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<(), String> {
    let _guard = lock_store(&state)?;
    with_store(&app, |path| clear_variables_at(path, &profile_name))
}

#[tauri::command]
async fn preview_import(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<Option<ImportPreview>, String> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Environment files", &["env"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let file_path = file
        .into_path()
        .map_err(|_| "Unable to access the selected environment file.".to_string())?;
    let content = fs::read_to_string(file_path)
        .map_err(|_| "Unable to read the selected file as UTF-8 text.".to_string())?;
    let parsed = parse_env(&content);
    let store_path = app_store_path(&app)?;
    let existing = {
        let _guard = lock_store(&state)?;
        let store = load_store(&store_path)?;
        crate::domain::effective_variables(&store.profiles, &profile_name)?
    };
    let (_, changes) = merge_import(&existing, &parsed.variables);
    let token = Uuid::new_v4().to_string();
    let preview = ImportPreview {
        token: token.clone(),
        added: changes.added,
        updated: changes.updated,
        invalid_lines: parsed.invalid_lines.clone(),
    };
    state
        .pending_imports
        .lock()
        .map_err(|_| {
            "Import preview state is unavailable. Restart the app and try again.".to_string()
        })?
        .insert(
            token,
            PendingImport {
                profile_name,
                variables: parsed.variables,
                invalid_lines: parsed.invalid_lines,
            },
        );
    Ok(Some(preview))
}

#[tauri::command]
fn discard_import(state: State<'_, AppState>, token: String) -> Result<(), String> {
    state
        .pending_imports
        .lock()
        .map_err(|_| {
            "Import preview state is unavailable. Restart the app and try again.".to_string()
        })?
        .remove(&token);
    Ok(())
}

#[tauri::command]
fn apply_import(
    app: AppHandle,
    state: State<'_, AppState>,
    token: String,
) -> Result<ImportResult, String> {
    let _store_guard = lock_store(&state)?;
    let path = app_store_path(&app)?;
    let mut pending = state.pending_imports.lock().map_err(|_| {
        "Import preview state is unavailable. Restart the app and try again.".to_string()
    })?;
    let batch = pending
        .get(&token)
        .cloned()
        .ok_or_else(|| "This import preview expired. Choose the file again.".to_string())?;
    let result = commit_import_at(&path, &batch)?;
    pending.remove(&token);
    Ok(result)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            set_active_profile,
            delete_profile,
            list_variables,
            save_variable,
            delete_variable,
            clear_variables,
            preview_import,
            apply_import,
            discard_import,
            get_shell_integrations,
            set_shell_integration
        ])
        .run(tauri::generate_context!())
        .expect("error while running environment manager");
}
