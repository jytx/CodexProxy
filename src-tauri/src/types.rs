use serde::{Deserialize, Serialize};

/// Tauri 前端传入的代理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub name: String,
    pub port: u16,
    /// 上游 API 完整地址，如 https://api.minimaxi.com/v1
    pub upstream_url: String,
    pub api_key: String,
    /// 模型名称覆盖（非空时替换请求中的模型名）
    pub model: String,
}

/// 代理运行状态（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub port: u16,
    pub upstream_url: String,
    pub model: String,
    pub name: String,
    pub requests_handled: u64,
}

/// 代理请求日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyLogEntry {
    /// 时间戳（毫秒）
    pub ts: i64,
    /// 请求方法
    pub method: String,
    /// 原始请求路径（如 http://localhost:port/v1/responses）
    pub path: String,
    /// 目标上游完整 URL（如 https://api.example.com/v1/chat/completions）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_url: Option<String>,
    /// 实际使用的模型（代理覆盖后）
    pub model: String,
    /// HTTP 状态码
    pub status: u16,
    /// 请求耗时（毫秒）
    pub duration_ms: u64,
    /// 是否流式请求
    pub is_stream: bool,
    /// 错误信息
    pub error: Option<String>,
    /// 原始请求体（Codex 发来的完整请求）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    /// 实际发送到上游的请求体（协议转换后的完整请求）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_request_body: Option<String>,
    /// 响应体（非流式时记录完整响应）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
}

/// 日志文件元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileInfo {
    /// 日志文件绝对路径
    pub file_path: String,
    /// 文件大小（字节）
    pub file_size_bytes: u64,
    /// 持久化的日志条目数
    pub entry_count: usize,
}

// ==================== Responses API 请求类型 ====================

/// Responses API 请求体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResponsesRequest {
    pub model: String,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Responses API 输入消息（从 input 数组解析）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResponsesInputMessage {
    #[serde(rename = "type")]
    pub msg_type: Option<String>,
    pub role: Option<String>,
    pub content: Option<serde_json::Value>,
    pub call_id: Option<String>,
    pub output: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
    pub id: Option<String>,
    pub status: Option<String>,
}

// ==================== Chat Completions API 类型 ====================

/// Chat Completions 响应体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatCompletionsResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatChoice {
    pub index: i32,
    pub message: Option<ChatMessage>,
    pub delta: Option<ChatDelta>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatToolCall {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub index: Option<i32>,
    pub function: ChatFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}
