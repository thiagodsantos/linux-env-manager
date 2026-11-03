use crate::models::PendingImport;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) pending_imports: Mutex<HashMap<String, PendingImport>>,
    pub(crate) store_lock: Mutex<()>,
}

pub(crate) fn lock_store(state: &AppState) -> Result<MutexGuard<'_, ()>, String> {
    state.store_lock.lock()
        .map_err(|_| "Variable store is unavailable. Restart the app and try again.".to_string())
}
