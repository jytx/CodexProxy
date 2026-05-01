use crate::convert::*;
use crate::log_writer::{self, ProxyLogState};
use crate::stream::handle_responses_stream;
use crate::types::*;
use axum::extract::{OriginalUri, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::Manager;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

/// 代理共享状态
#[derive(Debug)]
pub struct ProxyState {
    pub config: ProxyConfig,
    pub running: bool,
    pub requests_handled: AtomicU64,
    pub shutdown_tx: Option<tokio::task::JoinHandle<()>>,
    /// 日志状态（内存缓冲 + 磁盘持久化 + 事件发射）
    pub log_state: Arc<Mutex<ProxyLogState>>,
}

impl ProxyState {
    pub fn new(config: ProxyConfig) -> Self {
        Self {
            config,
            running: false,
            requests_handled: AtomicU64::new(0),
            shutdown_tx: None,
            log_state: Arc::new(Mutex::new(ProxyLogState::new())),
        }
    }
}

/// 启动代理 HTTP 服务
pub async fn start_proxy_server(
    state: Arc<Mutex<ProxyState>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let (port, upstream_url, api_key, model, name) = {
        let s = state.lock().await;
        if s.running {
            return Err("代理已在运行中".to_string());
        }
        (
            s.config.port,
            s.config.upstream_url.clone(),
            s.config.api_key.clone(),
            s.config.model.clone(),
            s.config.name.clone(),
        )
    };

    // 初始化日志写入器
    {
        let app_data = app_handle.path().app_data_dir()
            .map_err(|e| format!("获取 app_data_dir 失败: {}", e))?;
        let s = state.lock().await;
        let log_state_ref = &s.log_state;
        {
            let inner = log_state_ref.lock().await;
            log_writer::init_log_writer(&inner, app_data, app_handle.clone());
        }
        // 从文件加载历史日志
        log_writer::load_history(log_state_ref).await;
    }

    let log_state = {
        let s = state.lock().await;
        s.log_state.clone()
    };

    let app_state = Arc::new(AppState {
        name,
        port,
        upstream_url,
        api_key,
        model,
        client: Client::new(),
        proxy_state: state.clone(),
        log_state,
        shutdown_tx: Mutex::new(None),
    });

    let app = Router::new()
        .route("/v1/responses", post(handle_responses))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/responses", post(handle_responses))
        .route("/chat/completions", post(handle_chat_completions))
        .route("/api/coding/paas/v4/responses", post(handle_responses))
        .route("/api/coding/paas/v4/chat/completions", post(handle_chat_completions))
        .route("/health", axum::routing::get(health_check))
        .layer(CorsLayer::permissive())
        .with_state(app_state.clone());

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("绑定端口 {} 失败: {}", port, e))?;

    log::info!("代理服务启动: {} → upstream {}", addr, app_state.upstream_url);

    {
        let mut s = state.lock().await;
        s.running = true;
    }

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            .ok();
    });

    {
        let mut s = state.lock().await;
        s.shutdown_tx = Some(handle);
    }
    app_state.shutdown_tx.lock().await.replace(tx);

    Ok(())
}

/// 停止代理服务
pub async fn stop_proxy_server(state: Arc<Mutex<ProxyState>>) -> Result<(), String> {
    let mut s = state.lock().await;
    if !s.running {
        return Err("代理未在运行".to_string());
    }
    s.running = false;
    if let Some(handle) = s.shutdown_tx.take() {
        handle.abort();
    }
    log::info!("代理服务已停止");
    Ok(())
}

/// 应用内部状态
struct AppState {
    name: String,
    port: u16,
    upstream_url: String,
    api_key: String,
    model: String,
    client: Client,
    proxy_state: Arc<Mutex<ProxyState>>,
    log_state: Arc<Mutex<ProxyLogState>>,
    shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl AppState {
    fn upstream_chat_url(&self) -> String {
        format!("{}/chat/completions", self.upstream_url.trim_end_matches('/'))
    }
}

/// 健康检查
async fn health_check() -> impl IntoResponse {
    axum::Json(json!({ "status": "ok" }))
}

/// 完整请求体捕获（无截断）
fn full_body(data: &[u8]) -> Option<String> {
    if data.is_empty() { return None; }
    Some(String::from_utf8_lossy(data).to_string())
}

/// 处理 /responses 请求（Codex → 代理 → 协议转换 → 上游 /chat/completions）
async fn handle_responses(
    OriginalUri(uri): OriginalUri,
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let start = std::time::Instant::now();

    // 保存原始请求体（完整，无截断）
    let req_body = full_body(&body);

    let resp_req: ResponsesRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_REQUEST, &format!("无效 JSON: {}", e));
        }
    };

    let req_model = resp_req.model.clone();
    let is_stream = resp_req.stream.unwrap_or(false);

    // 实际使用的模型：优先使用代理配置的覆盖模型
    let effective_model = if state.model.is_empty() {
        req_model.clone()
    } else {
        state.model.clone()
    };

    log::info!("[resp→chat] model={} stream={} override={}", req_model, is_stream, state.model);

    let chat_body = responses_to_chat(&resp_req, &effective_model);

    // 实际发送到上游的请求体（协议转换后）
    let actual_req_body = Some(chat_body.to_string());

    let upstream_url = state.upstream_chat_url();
    increment_requests(&state.proxy_state).await;

    let result = if is_stream {
        handle_responses_stream(&state.client, &upstream_url, &state.api_key, chat_body, &effective_model).await
    } else {
        handle_responses_non_stream(&state, &upstream_url, &state.api_key, chat_body, &effective_model).await
    };

    // 记录日志（显示实际使用的模型）
    let status_code = result.status().as_u16();
    let origin_url = format!("http://127.0.0.1:{}{}", state.port, uri.path());
    log_writer::push_log(&state.log_state, ProxyLogEntry {
        ts: chrono::Utc::now().timestamp_millis(),
        method: "POST".into(),
        path: origin_url,
        upstream_url: Some(upstream_url.clone()),
        model: effective_model.clone(),
        status: status_code,
        duration_ms: start.elapsed().as_millis() as u64,
        is_stream,
        error: if status_code >= 400 { Some(format!("HTTP {}", status_code)) } else { None },
        request_body: req_body,
        actual_request_body: actual_req_body,
        response_body: None,
    }).await;

    result
}

/// 处理 /chat/completions 请求（透传到上游）
async fn handle_chat_completions(
    OriginalUri(uri): OriginalUri,
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> axum::response::Response {
    let start = std::time::Instant::now();
    let upstream_url = state.upstream_chat_url();
    let origin_url = format!("http://127.0.0.1:{}{}", state.port, uri.path());

    let req_body = full_body(&body);

    increment_requests(&state.proxy_state).await;

    let resp = match state.client
        .post(&upstream_url)
        .header("Authorization", format!("Bearer {}", state.api_key))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let err_msg = format!("上游错误: {}", e);
            log_writer::push_log(&state.log_state, ProxyLogEntry {
                ts: chrono::Utc::now().timestamp_millis(),
                method: "POST".into(),
                path: origin_url.clone(),
                upstream_url: Some(upstream_url.clone()),
                model: state.model.clone(),
                status: 502,
                duration_ms: start.elapsed().as_millis() as u64,
                is_stream: false,
                error: Some(err_msg.clone()),
                request_body: req_body.clone(),
                actual_request_body: req_body,
                response_body: None,
            }).await;
            return error_response(StatusCode::BAD_GATEWAY, &err_msg);
        }
    };

    let status = resp.status();
    let resp_body = resp.bytes().await.unwrap_or_default();

    let axum_status = StatusCode::from_u16(status.as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);

    let status_code = axum_status.as_u16();
    let resp_body_str = full_body(&resp_body);

    log_writer::push_log(&state.log_state, ProxyLogEntry {
        ts: chrono::Utc::now().timestamp_millis(),
        method: "POST".into(),
        path: origin_url,
        upstream_url: Some(upstream_url),
        model: state.model.clone(),
        status: status_code,
        duration_ms: start.elapsed().as_millis() as u64,
        is_stream: false,
        error: if status_code >= 400 { Some(format!("HTTP {}", status_code)) } else { None },
        request_body: req_body.clone(),
        actual_request_body: req_body,
        response_body: resp_body_str,
    }).await;

    (
        axum_status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        resp_body.to_vec(),
    )
        .into_response()
}

/// 非流式：协议转换请求 → 上游 → 转回响应
async fn handle_responses_non_stream(
    state: &AppState,
    url: &str,
    api_key: &str,
    chat_body: serde_json::Value,
    model: &str,
) -> axum::response::Response {
    let resp = match state.client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&chat_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_GATEWAY, &format!("上游错误: {}", e));
        }
    };

    let status = resp.status();
    let resp_body = resp.bytes().await.unwrap_or_default();

    let axum_status = StatusCode::from_u16(status.as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);

    if axum_status != StatusCode::OK {
        return (axum_status, [("content-type", "application/json")], resp_body.to_vec()).into_response();
    }

    let chat_resp: ChatCompletionsResponse = match serde_json::from_slice(&resp_body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_GATEWAY, &format!("解析上游响应失败: {}", e));
        }
    };

    let responses_resp = chat_to_responses(&chat_resp, model);
    (StatusCode::OK, axum::Json(responses_resp)).into_response()
}

fn error_response(status: StatusCode, msg: &str) -> axum::response::Response {
    (
        status,
        [("content-type", "application/json")],
        json!({ "error": { "message": msg } }).to_string(),
    )
        .into_response()
}

async fn increment_requests(state: &Arc<Mutex<ProxyState>>) {
    let s = state.lock().await;
    s.requests_handled.fetch_add(1, Ordering::Relaxed);
}
