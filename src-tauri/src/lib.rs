mod convert;
mod codex_config;
mod log_writer;
mod proxy;
mod stream;
mod types;

use proxy::{start_proxy_server, stop_proxy_server, ProxyState};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;
use types::{LogFileInfo, ProxyConfig, ProxyLogEntry, ProxyStatus};

/// 代理状态全局句柄
struct AppCtx {
    state: Arc<Mutex<ProxyState>>,
}

/// 获取代理状态
#[tauri::command]
async fn get_proxy_status(ctx: tauri::State<'_, AppCtx>) -> Result<ProxyStatus, String> {
    let s = ctx.state.lock().await;
    Ok(ProxyStatus {
        running: s.running,
        port: s.config.port,
        upstream_url: s.config.upstream_url.clone(),
        model: s.config.model.clone(),
        name: s.config.name.clone(),
        requests_handled: s.requests_handled.load(Ordering::Relaxed),
    })
}

/// 获取代理日志
#[tauri::command]
async fn get_proxy_logs(ctx: tauri::State<'_, AppCtx>) -> Result<Vec<ProxyLogEntry>, String> {
    let s = ctx.state.lock().await;
    Ok(log_writer::get_logs(&s.log_state).await)
}

/// 获取日志文件元信息
#[tauri::command]
async fn get_log_file_info(ctx: tauri::State<'_, AppCtx>) -> Result<Option<LogFileInfo>, String> {
    let s = ctx.state.lock().await;
    Ok(log_writer::get_file_info(&s.log_state).await)
}

/// 在 Finder 中打开日志文件所在目录
#[tauri::command]
async fn open_log_folder(ctx: tauri::State<'_, AppCtx>) -> Result<(), String> {
    let s = ctx.state.lock().await;
    let file_info = log_writer::get_file_info(&s.log_state).await
        .ok_or("日志文件未初始化")?;

    // 使用 macOS 的 open 命令定位到文件
    std::process::Command::new("open")
        .arg("-R")
        .arg(&file_info.file_path)
        .spawn()
        .map_err(|e| format!("打开文件夹失败: {}", e))?;
    Ok(())
}

/// 启动代理
#[tauri::command]
async fn start_proxy(
    config: ProxyConfig,
    ctx: tauri::State<'_, AppCtx>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let sync_name = config.name.clone();
    let sync_model = config.model.clone();
    let sync_port = config.port;

    {
        let mut s = ctx.state.lock().await;
        s.config = config;
    }
    start_proxy_server(ctx.state.clone(), app).await?;

    // 自动同步 Codex 配置（model、model_provider、profiles）
    if let Err(e) = codex_config::sync_codex_config(&sync_name, &sync_model, sync_port) {
        log::warn!("同步 Codex 配置失败: {}", e);
    }

    Ok("代理已启动".to_string())
}

/// 停止代理
#[tauri::command]
async fn stop_proxy(ctx: tauri::State<'_, AppCtx>) -> Result<String, String> {
    stop_proxy_server(ctx.state.clone()).await?;
    Ok("代理已停止".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_state = ProxyState::new(ProxyConfig {
        name: String::new(),
        port: 9090,
        upstream_url: String::new(),
        api_key: String::new(),
        model: String::new(),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(AppCtx {
            state: Arc::new(Mutex::new(initial_state)),
        })
        .invoke_handler(tauri::generate_handler![
            get_proxy_status,
            get_proxy_logs,
            get_log_file_info,
            open_log_folder,
            start_proxy,
            stop_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("启动 CodexProxy 失败");
}
