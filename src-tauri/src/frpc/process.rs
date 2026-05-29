use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::error::{AppError, AppResult};

/// frpc 配置文件目录：data_dir/frpc/
pub fn frpc_config_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("frpc")
}

/// 安全路径检查（已有文件）：canonicalize + 前缀检查。
/// 用于 read/write/delete 操作，文件必须已存在。
pub fn safe_config_path(data_dir: &Path, file_name: &str) -> AppResult<PathBuf> {
    let dir = frpc_config_dir(data_dir);
    let path = dir.join(file_name);
    let canon = std::fs::canonicalize(&path).map_err(|_| {
        AppError::config(
            "frpc_file_not_found",
            serde_json::json!({ "path": path.display().to_string() }),
        )
    })?;
    let dir_canon = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
    if !canon.starts_with(&dir_canon) {
        return Err(AppError::config("frpc_path_traversal", serde_json::json!({})));
    }
    Ok(canon)
}

/// 安全路径检查（新建文件）：canonicalize 父目录 + 前缀检查。
/// 用于 create 操作，文件尚不存在。
pub fn safe_config_dir(data_dir: &Path) -> AppResult<PathBuf> {
    let dir = frpc_config_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let dir_canon = std::fs::canonicalize(&dir)?;
    Ok(dir_canon)
}

/// 启动 frpc 子进程
pub fn spawn_frpc(binary: &Path, config_path: &Path) -> AppResult<Child> {
    let child = Command::new(binary)
        .arg("-c")
        .arg(config_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            AppError::config(
                "frpc_spawn_failed",
                serde_json::json!({ "err": e.to_string() }),
            )
        })?;
    Ok(child)
}

/// 桌面端：自动探测 frpc 二进制路径
#[cfg(not(target_os = "android"))]
pub fn find_frpc_binary() -> Option<PathBuf> {
    // 1. which / where
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    if let Ok(output) = std::process::Command::new(which_cmd).arg("frpc").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let path_str = path_str.trim();
            if !path_str.is_empty() {
                let first_line = path_str.split('\n').next().unwrap_or(path_str).trim();
                let path = PathBuf::from(first_line);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }

    // 2. Common paths
    let candidates: Vec<PathBuf> = vec![
        PathBuf::from("/usr/local/bin/frpc"),
        PathBuf::from("/usr/bin/frpc"),
        PathBuf::from("/opt/homebrew/bin/frpc"),
    ];
    if let Some(home) = dirs::home_dir() {
        let mut candidates = candidates;
        candidates.push(home.join(".local/bin/frpc"));
        return candidates.into_iter().find(|p| p.is_file());
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Android 端：bundled libfrpc.so 路径
#[cfg(target_os = "android")]
pub fn find_frpc_binary() -> Option<PathBuf> {
    // /proc/self/maps 列出本进程加载的所有 .so，从中提取 native lib 目录
    if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
        for line in maps.lines() {
            if line.contains("librssh_lib.so") {
                // line format: "7f8...000 /data/app/com.rssh.app-XXX/lib/arm64/librssh_lib.so"
                if let Some(path) = line.splitn(2, '/').nth(1) {
                    let full = PathBuf::from(format!("/{path}"));
                    if let Some(parent) = full.parent() {
                        let candidate = parent.join("libfrpc.so");
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
    }
    None
}

/// 解析二进制路径（优先用户设置，再自动探测）
pub fn resolve_frpc_binary(
    db: &crate::db::Db,
) -> AppResult<PathBuf> {
    // 用户手动设置的路径
    if let Ok(Some(path)) = crate::db::settings::get(db, "frpc_binary_path") {
        if !path.is_empty() {
            let p = PathBuf::from(&path);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    // 自动探测
    find_frpc_binary().ok_or_else(|| AppError::config("frpc_binary_not_found", serde_json::json!({})))
}

/// 自动启动一个 frpc 配置
pub fn auto_start_config(
    state: &crate::state::AppState,
    config: &crate::models::FrpcConfig,
) -> AppResult<()> {
    let binary = resolve_frpc_binary(&state.db)?;
    let config_path = safe_config_path(&state.data_dir, &config.file_name)?;
    let child = spawn_frpc(&binary, &config_path)?;
    let handle = super::handle::FrpcHandle::new(child, config.id.clone());
    let mut frpc = crate::error::locked(&state.active_frpc)?;
    frpc.insert(config.id.clone(), handle);
    super::android::start_frpc_notification(&config.id, &config.name);
    Ok(())
}
