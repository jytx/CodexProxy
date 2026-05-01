/** 代理配置项（持久化到 localStorage） */
export interface ProxyProfile {
  /** 唯一标识（crypto.randomUUID） */
  id: string;
  /** 配置名称 */
  name: string;
  /** 上游 API 完整地址，如 https://api.minimaxi.com/v1 */
  baseUrl: string;
  /** API Key 值 */
  apiKey: string;
  /** 模型名称覆盖（非空时替换请求中的模型名） */
  model: string;
  /** 本地监听端口 */
  port: number;
}

/** 代理运行状态（Tauri 命令返回） */
export interface ProxyStatus {
  running: boolean;
  port: number;
  upstream_url: string;
  model: string;
  name: string;
  requests_handled: number;
}

/** 应用主题 */
export type Theme = "light" | "dark" | "system";

/** 代理请求日志条目 */
export interface ProxyLogEntry {
  ts: number;
  method: string;
  /** 原始请求完整 URL（含域名端口） */
  path: string;
  /** 目标上游完整 URL */
  upstream_url: string | null;
  model: string;
  status: number;
  duration_ms: number;
  is_stream: boolean;
  error: string | null;
  /** 原始请求体（Codex 发来的完整请求） */
  request_body: string | null;
  /** 实际发送到上游的请求体（协议转换后） */
  actual_request_body: string | null;
  /** 响应体（非流式时记录完整响应） */
  response_body: string | null;
}

/** 日志文件元信息 */
export interface LogFileInfo {
  /** 日志文件绝对路径 */
  file_path: string;
  /** 文件大小（字节） */
  file_size_bytes: number;
  /** 持久化的日志条目数 */
  entry_count: number;
}
