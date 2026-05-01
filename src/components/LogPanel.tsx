import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ProxyLogEntry, LogFileInfo } from "../types/app";
import { JsonViewer } from "./JsonViewer";

interface LogPanelProps {
  open: boolean;
  onClose: () => void;
}

/** 格式化时间戳（含月日） */
function formatTime(ts: number): string {
  const d = new Date(ts);
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const mi = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${mm}-${dd} ${hh}:${mi}:${ss}`;
}

/** 格式化耗时 */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/** 格式化文件大小 */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** 状态码颜色 */
function statusColor(status: number): string {
  if (status < 300) return "var(--status-ok, #34d399)";
  if (status < 400) return "var(--status-warn, #fbbf24)";
  return "var(--status-err, #f26a77)";
}

/** 下载 JSON 内容为文件 */
function downloadJson(content: string, filename: string) {
  let formatted: string;
  try {
    formatted = JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    formatted = content;
  }
  const blob = new Blob([formatted], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

/** 复制文本到剪贴板 */
async function copyText(content: string): Promise<boolean> {
  try {
    let formatted: string;
    try {
      formatted = JSON.stringify(JSON.parse(content), null, 2);
    } catch {
      formatted = content;
    }
    await navigator.clipboard.writeText(formatted);
    return true;
  } catch {
    return false;
  }
}

export function LogPanel({ open, onClose }: LogPanelProps) {
  const [logs, setLogs] = useState<ProxyLogEntry[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const [selected, setSelected] = useState<ProxyLogEntry | null>(null);
  const [sortAsc, setSortAsc] = useState(false);
  const [logInfo, setLogInfo] = useState<LogFileInfo | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  /** 加载历史日志 */
  const loadHistory = useCallback(async () => {
    try {
      const history = await invoke<ProxyLogEntry[]>("get_proxy_logs");
      setLogs(history);
    } catch { /* ignore */ }
  }, []);

  /** 加载日志文件信息 */
  const loadLogInfo = useCallback(async () => {
    try {
      const info = await invoke<LogFileInfo | null>("get_log_file_info");
      setLogInfo(info);
    } catch { /* ignore */ }
  }, []);

  /** 监听实时日志事件 */
  useEffect(() => {
    if (!open) return;

    loadHistory();
    loadLogInfo();

    let cancelled = false;
    listen<ProxyLogEntry>("proxy-log", (event) => {
      if (cancelled) return;
      setLogs((prev) => [...prev, event.payload]);
      // 刷新文件信息
      loadLogInfo();
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
      } else {
        unlistenRef.current = unlisten;
      }
    });

    return () => {
      cancelled = true;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, [open, loadHistory, loadLogInfo]);

  /** 自动滚动到底部 */
  useEffect(() => {
    if (autoScroll && listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [logs, autoScroll, sortAsc]);

  const handleScroll = () => {
    if (!listRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = listRef.current;
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 40);
  };

  const handleClear = () => setLogs([]);
  const handleToggleSort = () => setSortAsc((prev) => !prev);
  const handleOpenFolder = async () => {
    try { await invoke("open_log_folder"); } catch { /* ignore */ }
  };

  /** 显示消息提示（2秒后自动消失） */
  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 2000);
  };

  /** 下载并提示 */
  const handleDownload = (content: string, filename: string) => {
    downloadJson(content, filename);
    showToast("已下载 " + filename);
  };

  /** 复制并提示 */
  const handleCopy = async (content: string, label: string) => {
    const ok = await copyText(content);
    showToast(ok ? `已复制${label}` : "复制失败");
  };

  /** 排序后的日志 */
  const displayLogs = sortAsc ? logs : [...logs].reverse();

  if (!open) return null;

  return (
    <>
      <div className="logOverlay" onClick={onClose}>
        <div className="logPanel" onClick={(e) => e.stopPropagation()}>
          <div className="logHeader">
            <span className="logTitle">请求日志</span>
            <div className="logActions">
              <span className="logCount">{logs.length} 条</span>
              {logInfo && (
                <span className="logFileInfo">
                  {formatFileSize(logInfo.file_size_bytes)} · {logInfo.entry_count} 条持久化
                </span>
              )}
              <button className="btn btnGhost btnSm" onClick={handleOpenFolder}>打开文件夹</button>
              <button className="btn btnGhost btnSm" onClick={handleClear}>清空</button>
              <button className="btn btnGhost btnSm" onClick={onClose}>关闭</button>
            </div>
          </div>

          <div className="logList" ref={listRef} onScroll={handleScroll}>
            {displayLogs.length === 0 ? (
              <div className="logEmpty">暂无请求日志，启动代理并发送请求后将在此实时显示</div>
            ) : (
              displayLogs.map((log, i) => (
                <div
                  key={i}
                  className={`logEntry ${log.error ? "logError" : ""}`}
                  onClick={() => setSelected(log)}
                >
                  <span className="logTime">{formatTime(log.ts)}</span>
                  <span className="logMethod">{log.method}</span>
                  <span className="logPath">{log.path.replace(/^https?:\/\/[^/]+/, "")}</span>
                  <span className="logModel">{log.model}</span>
                  <span className="logStream">{log.is_stream ? "SSE" : "sync"}</span>
                  <span className="logStatus" style={{ color: statusColor(log.status) }}>
                    {log.status}
                  </span>
                  <span className="logDuration">{formatDuration(log.duration_ms)}</span>
                  {log.error && <span className="logErrMsg">{log.error}</span>}
                </div>
              ))
            )}
          </div>

          {/* 排序按钮 */}
          <div className="logFooter">
            <button className="btn btnGhost btnSm" onClick={handleToggleSort}>
              时间排序 {sortAsc ? "↑ 最早" : "↓ 最新"}
            </button>
          </div>
        </div>
      </div>

      {/* 详情弹窗 */}
      {selected && (
        <div className="logDetailOverlay" onClick={() => setSelected(null)}>
          <div className="logDetailPanel" onClick={(e) => e.stopPropagation()}>
            <div className="logDetailHeader">
              <span className="logDetailTitle">请求详情</span>
              <button className="btnIcon" onClick={() => setSelected(null)}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18" />
                  <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </div>

            <div className="logDetailGrid">
              <span className="logDetailLabel">时间</span>
              <span className="logDetailValue">{new Date(selected.ts).toLocaleString("zh-CN")}</span>

              <span className="logDetailLabel">原路径</span>
              <span className="logDetailValue logDetailUrl">{selected.method} {selected.path}</span>

              {selected.upstream_url && (
                <>
                  <span className="logDetailLabel">目标路径</span>
                  <span className="logDetailValue logDetailUrl">{selected.upstream_url}</span>
                </>
              )}

              <span className="logDetailLabel">模型</span>
              <span className="logDetailValue">{selected.model}</span>

              <span className="logDetailLabel">状态</span>
              <span className="logDetailValue" style={{ color: statusColor(selected.status) }}>
                {selected.status}
              </span>

              <span className="logDetailLabel">耗时</span>
              <span className="logDetailValue">{formatDuration(selected.duration_ms)}</span>

              <span className="logDetailLabel">模式</span>
              <span className="logDetailValue">{selected.is_stream ? "流式 (SSE)" : "非流式"}</span>

              {selected.error && (
                <>
                  <span className="logDetailLabel">错误</span>
                  <span className="logDetailValue" style={{ color: "#f26a77" }}>{selected.error}</span>
                </>
              )}
            </div>

            {selected.request_body && (
              <div>
                <div className="logDetailBodyLabel">
                  原始请求体
                  <div className="logBodyActions">
                    <button className="btn btnGhost btnSm logDlBtn" onClick={() => handleCopy(selected.request_body!, "原始请求体")}>
                      复制
                    </button>
                    <button className="btn btnGhost btnSm logDlBtn" onClick={() => handleDownload(selected.request_body!, "original-request.json")}>
                      下载
                    </button>
                  </div>
                </div>
                <JsonViewer src={selected.request_body} />
              </div>
            )}

            {selected.actual_request_body && (
              <div>
                <div className="logDetailBodyLabel">
                  实际请求体
                  <div className="logBodyActions">
                    <button className="btn btnGhost btnSm logDlBtn" onClick={() => handleCopy(selected.actual_request_body!, "实际请求体")}>
                      复制
                    </button>
                    <button className="btn btnGhost btnSm logDlBtn" onClick={() => handleDownload(selected.actual_request_body!, "actual-request.json")}>
                      下载
                    </button>
                  </div>
                </div>
                <JsonViewer src={selected.actual_request_body} />
              </div>
            )}

            {selected.response_body && (
              <div>
                <div className="logDetailBodyLabel">
                  响应体
                  <div className="logBodyActions">
                    <button className="btn btnGhost btnSm logDlBtn" onClick={() => handleCopy(selected.response_body!, "响应体")}>
                      复制
                    </button>
                    <button className="btn btnGhost btnSm logDlBtn" onClick={() => handleDownload(selected.response_body!, "response.json")}>
                      下载
                    </button>
                  </div>
                </div>
                <JsonViewer src={selected.response_body} />
              </div>
            )}

            {!selected.request_body && !selected.actual_request_body && !selected.response_body && (
              <div style={{ color: "var(--subtle)", fontSize: "0.82rem", textAlign: "center", padding: "12px 0" }}>
                无请求/响应体数据
              </div>
            )}
          </div>
        </div>
      )}

      {/* 消息提示 */}
      {toast && (
        <div className="logToast">{toast}</div>
      )}
    </>
  );
}
