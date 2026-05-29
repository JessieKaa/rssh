use serde::Serialize;
use serde_json::json;
use tauri::State;

use crate::error::{locked, AppError, AppResult};
use crate::frpc::android;
use crate::frpc::handle::FrpcHandle;
use crate::frpc::process;
use crate::models::FrpcConfig;
use crate::state::AppState;

// ── Response types ──

#[derive(Serialize)]
pub struct FrpcStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

#[derive(Serialize)]
pub struct FrpcBinaryInfo {
    pub found: bool,
    pub path: String,
    pub source: String,
}

// ── TOML template ──

const TOML_TEMPLATE: &str = r#"# frpc client configuration
# Documentation: https://gofrp.org/docs/
serverAddr = ""
serverPort = 7000
# auth.token = ""

[[proxies]]
name = "ssh"
type = "tcp"
localIP = "127.0.0.1"
localPort = 22
remotePort = 6000
"#;

// ── CRUD ──

#[tauri::command]
pub fn list_frpc_configs(state: State<AppState>) -> Result<Vec<FrpcConfig>, AppError> {
    crate::db::frpc::list(&state.db)
}

#[tauri::command]
pub fn get_frpc_config(state: State<AppState>, id: String) -> Result<FrpcConfig, AppError> {
    crate::db::frpc::get(&state.db, &id)
}

#[tauri::command]
pub fn create_frpc_config(state: State<AppState>, name: String) -> AppResult<FrpcConfig> {
    let id = uuid::Uuid::new_v4().to_string();
    let file_name = format!("{}.toml", uuid::Uuid::new_v4());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Write TOML template file first (before DB insert)
    let dir = process::safe_config_dir(&state.data_dir)?;
    let file_path = dir.join(&file_name);
    std::fs::write(&file_path, TOML_TEMPLATE)?;

    let config = FrpcConfig {
        id,
        name,
        file_name,
        enabled: false,
        created_at: now,
    };

    // Insert DB record; on failure, rollback the file
    if let Err(e) = crate::db::frpc::insert(&state.db, &config) {
        let _ = std::fs::remove_file(&file_path);
        return Err(e);
    }

    Ok(config)
}

#[tauri::command]
pub fn delete_frpc_config(state: State<AppState>, id: String) -> AppResult<()> {
    let config = crate::db::frpc::get(&state.db, &id)?;
    // Delete file from disk
    if let Ok(path) = process::safe_config_path(&state.data_dir, &config.file_name) {
        let _ = std::fs::remove_file(path);
    }
    crate::db::frpc::delete(&state.db, &id)
}

// ── TOML file operations ──

#[tauri::command]
pub fn read_frpc_toml(state: State<AppState>, file_name: String) -> AppResult<String> {
    let path = process::safe_config_path(&state.data_dir, &file_name)?;
    Ok(std::fs::read_to_string(path)?)
}

#[tauri::command]
pub fn write_frpc_toml(state: State<AppState>, file_name: String, content: String) -> AppResult<()> {
    let path = process::safe_config_path(&state.data_dir, &file_name)?;
    Ok(std::fs::write(path, content)?)
}

#[tauri::command]
pub fn rename_frpc_config(state: State<AppState>, id: String, new_name: String) -> AppResult<()> {
    let mut config = crate::db::frpc::get(&state.db, &id)?;
    config.name = new_name;
    crate::db::frpc::update(&state.db, &config)
}

#[tauri::command]
pub fn toggle_frpc_enabled(state: State<AppState>, config_id: String, enabled: bool) -> AppResult<()> {
    crate::db::frpc::toggle_enabled(&state.db, &config_id, enabled)
}

#[tauri::command]
pub fn get_frpc_auto_start_errors(state: State<AppState>) -> Vec<String> {
    locked(&state.frpc_auto_start_errors)
        .map(|errors| errors.clone())
        .unwrap_or_default()
}

// ── Process management ──

#[tauri::command]
pub async fn frpc_start(state: State<'_, AppState>, config_id: String) -> AppResult<String> {
    // Resolve binary + config path OUTSIDE the lock
    let binary = process::resolve_frpc_binary(&state.db)?;
    let config = crate::db::frpc::get(&state.db, &config_id)?;
    let config_path = process::safe_config_path(&state.data_dir, &config.file_name)?;
    let child = process::spawn_frpc(&binary, &config_path)?;

    // Lock only for check-and-insert
    let mut frpc = locked(&state.active_frpc)?;
    if frpc.contains_key(&config_id) {
        // std::process::Child::drop does NOT kill — must explicit kill+wait
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
        return Err(AppError::config("frpc_already_running", json!({})));
    }
    let handle = FrpcHandle::new(child, config_id.clone());
    frpc.insert(config_id.clone(), handle);
    android::start_frpc_notification(&config_id, &config.name);
    Ok(config_id)
}

#[tauri::command]
pub async fn frpc_stop(state: State<'_, AppState>, config_id: String) -> AppResult<()> {
    let handle = {
        let mut frpc = locked(&state.active_frpc)?;
        frpc.remove(&config_id)
    };
    let handle = handle.ok_or_else(|| AppError::not_found("frpc_not_running", json!({})))?;
    // kill + wait in background to avoid blocking
    tokio::task::spawn_blocking(move || handle.stop());

    // Clean dead processes and update notification
    let alive = reap_dead_frpc(&state);
    if alive.is_empty() {
        android::stop_frpc_notification();
    } else {
        android::update_frpc_notification(&alive);
    }
    Ok(())
}

#[tauri::command]
pub fn frpc_status(state: State<AppState>, config_id: String) -> AppResult<FrpcStatus> {
    let alive = reap_dead_frpc(&state);
    if alive.is_empty() {
        android::stop_frpc_notification();
    } else {
        android::update_frpc_notification(&alive);
    }

    let frpc = locked(&state.active_frpc)?;
    match frpc.get(&config_id) {
        Some(handle) => Ok(FrpcStatus {
            running: handle.is_running(),
            pid: handle.pid(),
        }),
        None => Ok(FrpcStatus {
            running: false,
            pid: None,
        }),
    }
}

#[tauri::command]
pub fn frpc_logs(
    state: State<AppState>,
    config_id: String,
    limit: Option<usize>,
) -> AppResult<Vec<String>> {
    let frpc = locked(&state.active_frpc)?;
    match frpc.get(&config_id) {
        Some(handle) => Ok(handle.logs(limit.unwrap_or(100))),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn frpc_binary_info(state: State<AppState>) -> AppResult<FrpcBinaryInfo> {
    // Check user-configured path first
    if let Ok(Some(path)) = crate::db::settings::get(&state.db, "frpc_binary_path") {
        if !path.is_empty() {
            let p = std::path::PathBuf::from(&path);
            return Ok(FrpcBinaryInfo {
                found: p.is_file(),
                path,
                source: "setting".into(),
            });
        }
    }
    // Auto-detect
    match process::find_frpc_binary() {
        Some(p) => Ok(FrpcBinaryInfo {
            found: true,
            path: p.display().to_string(),
            source: "which".into(),
        }),
        None => Ok(FrpcBinaryInfo {
            found: false,
            path: String::new(),
            source: "none".into(),
        }),
    }
}

/// Remove dead frpc handles and return names of still-alive configs.
fn reap_dead_frpc(state: &AppState) -> Vec<String> {
    let mut frpc = match locked(&state.active_frpc) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let dead_ids: Vec<String> = frpc
        .iter()
        .filter(|(_, h)| !h.is_running())
        .map(|(id, _)| id.clone())
        .collect();
    for id in &dead_ids {
        frpc.remove(id);
    }
    frpc
        .iter()
        .filter_map(|(id, _)| crate::db::frpc::get(&state.db, id).ok().map(|c| c.name))
        .collect()
}
