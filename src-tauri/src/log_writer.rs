/// 日志持久化模块
/// 负责将代理请求日志写入 JSONL 文件，并提供文件元信息查询
use crate::types::*;
use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex as StdMutex;
use tauri::Emitter;
use tokio::sync::Mutex;

/// 内存缓冲区最大条数
const LOG_BUFFER_SIZE: usize = 200;

/// 日志文件写入器
#[derive(Debug)]
pub struct LogWriter {
    /// 当前会话的日志文件
    file: StdMutex<Option<fs::File>>,
    /// 日志文件路径
    file_path: PathBuf,
}

impl LogWriter {
    /// 创建新的日志写入器，在指定目录下创建以时间戳命名的 JSONL 文件
    pub fn new(log_dir: &PathBuf) -> Self {
        // 确保日志目录存在
        let _ = fs::create_dir_all(log_dir);

        let now = chrono::Local::now();
        let filename = format!("proxy-{}.jsonl", now.format("%Y%m%d-%H%M%S"));
        let file_path = log_dir.join(&filename);

        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .ok();

        Self {
            file: StdMutex::new(file),
            file_path,
        }
    }

    /// 追加一条日志到文件
    pub fn append(&self, entry: &ProxyLogEntry) -> Result<(), String> {
        let mut guard = self.file.lock().map_err(|e| format!("锁错误: {}", e))?;
        let file = guard.as_mut().ok_or("日志文件未打开")?;
        let line = serde_json::to_string(entry).map_err(|e| format!("序列化失败: {}", e))?;
        writeln!(file, "{}", line).map_err(|e| format!("写入失败: {}", e))?;
        Ok(())
    }

    /// 从当前会话文件加载所有日志条目
    pub fn load_today(&self) -> Result<Vec<ProxyLogEntry>, String> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("读取日志文件失败: {}", e))?;

        let mut entries = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<ProxyLogEntry>(line) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// 获取日志文件元信息
    pub fn file_info(&self) -> LogFileInfo {
        let metadata = fs::metadata(&self.file_path).ok();
        let file_size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        // 计算条目数（通过加载解析）
        let entry_count = self.load_today().map(|e| e.len()).unwrap_or(0);

        LogFileInfo {
            file_path: self.file_path.to_string_lossy().to_string(),
            file_size_bytes,
            entry_count,
        }
    }

    /// 获取日志文件路径（用于在 Finder 中显示）
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
}

/// 代理共享状态中的日志部分
#[derive(Debug)]
pub struct ProxyLogState {
    /// 内存缓冲区（用于实时显示）
    pub buffer: StdMutex<VecDeque<ProxyLogEntry>>,
    /// 文件写入器
    pub writer: StdMutex<Option<LogWriter>>,
    /// Tauri AppHandle（用于向前端发射事件）
    pub app_handle: StdMutex<Option<tauri::AppHandle>>,
}

impl ProxyLogState {
    pub fn new() -> Self {
        Self {
            buffer: StdMutex::new(VecDeque::with_capacity(LOG_BUFFER_SIZE)),
            writer: StdMutex::new(None),
            app_handle: StdMutex::new(None),
        }
    }
}

/// 初始化日志写入器（在 Tauri setup 时调用）
pub fn init_log_writer(log_state: &ProxyLogState, app_data_dir: PathBuf, app_handle: tauri::AppHandle) {
    let log_dir = app_data_dir.join("logs");
    let writer = LogWriter::new(&log_dir);

    if let Ok(mut w) = log_state.writer.lock() {
        *w = Some(writer);
    }
    if let Ok(mut h) = log_state.app_handle.lock() {
        *h = Some(app_handle);
    }
}

/// 追加日志条目：内存缓冲 + 磁盘持久化 + Tauri 事件发射
pub async fn push_log(
    log_state: &std::sync::Arc<Mutex<ProxyLogState>>,
    entry: ProxyLogEntry,
) {
    // 磁盘持久化 + Tauri 事件（在锁外操作）
    let (app, entry_clone) = {
        let s = log_state.lock().await;

        // 内存缓冲
        if let Ok(mut buf) = s.buffer.lock() {
            if buf.len() >= LOG_BUFFER_SIZE {
                buf.pop_front();
            }
            buf.push_back(entry.clone());
        }

        // 磁盘写入
        if let Ok(mut w_guard) = s.writer.lock() {
            if let Some(ref mut writer) = *w_guard {
                let _ = writer.append(&entry);
            }
        }

        let app = s.app_handle.lock().ok().and_then(|g| g.clone());
        (app, entry)
    };

    // Tauri 事件发射
    if let Some(app) = app {
        let _ = app.emit("proxy-log", &entry_clone);
    }
}

/// 获取内存缓冲区中的所有日志
pub async fn get_logs(log_state: &std::sync::Arc<Mutex<ProxyLogState>>) -> Vec<ProxyLogEntry> {
    let s = log_state.lock().await;
    let buf = s.buffer.lock().ok();
    match buf {
        Some(b) => b.iter().cloned().collect(),
        None => Vec::new(),
    }
}

/// 从文件加载历史日志到内存缓冲区
pub async fn load_history(log_state: &std::sync::Arc<Mutex<ProxyLogState>>) {
    let entries = {
        let s = log_state.lock().await;
        let w_guard = s.writer.lock().ok();
        match w_guard {
            Some(guard) => {
                match guard.as_ref() {
                    Some(writer) => writer.load_today().unwrap_or_default(),
                    None => Vec::new(),
                }
            }
            None => Vec::new(),
        }
    };

    if entries.is_empty() {
        return;
    }

    let s = log_state.lock().await;
    if let Ok(mut buf) = s.buffer.lock() {
        for entry in entries {
            if buf.len() >= LOG_BUFFER_SIZE {
                buf.pop_front();
            }
            buf.push_back(entry);
        }
    };
}

/// 获取日志文件元信息
pub async fn get_file_info(log_state: &std::sync::Arc<Mutex<ProxyLogState>>) -> Option<LogFileInfo> {
    let s = log_state.lock().await;
    let w_guard = s.writer.lock().ok()?;
    w_guard.as_ref().map(|writer| writer.file_info())
}
