import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ProxyProfile, ProxyStatus } from "../types/app";
import { useProxyStore } from "../hooks/useProxyStore";
import { ProxyCard } from "./ProxyCard";
import { ProxyModal } from "./ProxyModal";

export function ProxyPanel() {
  const { profiles, addProfile, updateProfile, deleteProfile, reorderProfiles } = useProxyStore();
  const [status, setStatus] = useState<ProxyStatus | null>(null);
  const [error, setError] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [editProfile, setEditProfile] = useState<ProxyProfile | undefined>(undefined);
  const [activeProfileId, setActiveProfileId] = useState<string | null>(null);

  const [statusRefreshKey, setStatusRefreshKey] = useState(0);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await invoke<ProxyStatus>("get_proxy_status");
      setStatus(s);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    refreshStatus();
    const id = setInterval(refreshStatus, 2000);
    return () => clearInterval(id);
  }, [refreshStatus, statusRefreshKey]);

  const isRunning = status?.running ?? false;
  const activeModel = status?.model ?? "";
  const activeName = status?.name ?? "";
  const activeRequests = status?.requests_handled ?? 0;
  const activePort = status?.port ?? 0;

  const handleStart = async (profile: ProxyProfile) => {
    setError("");
    try {
      if (isRunning) await invoke("stop_proxy");
      await invoke("start_proxy", {
        config: {
          name: profile.name, port: profile.port,
          upstream_url: profile.baseUrl, api_key: profile.apiKey, model: profile.model,
        },
      });
      setActiveProfileId(profile.id);
      setStatusRefreshKey(k => k + 1);
    } catch (e) { setError(String(e)); }
  };

  const handleStop = async () => {
    setError("");
    try {
      await invoke("stop_proxy");
      setActiveProfileId(null);
      setStatusRefreshKey(k => k + 1);
    } catch (e) { setError(String(e)); }
  };

  const handleOpenAdd = () => { setEditProfile(undefined); setModalOpen(true); };
  const handleOpenEdit = (p: ProxyProfile) => { setEditProfile(p); setModalOpen(true); };
  const handleModalSave = (data: Omit<ProxyProfile, "id">) => {
    if (editProfile) updateProfile(editProfile.id, data); else addProfile(data);
    setModalOpen(false); setEditProfile(undefined);
  };
  const handleModalClose = () => { setModalOpen(false); setEditProfile(undefined); };
  const handleDelete = (p: ProxyProfile) => {
    if (activeProfileId === p.id) handleStop();
    deleteProfile(p.id);
  };

  // ==================== 拖拽 ====================
  const [dragId, setDragId] = useState<string | null>(null);
  const [displayOrder, setDisplayOrder] = useState<ProxyProfile[]>([]);
  // 拖拽幽灵：通过 DOM clone 实现，保证与原卡片像素级一致
  const dragGhostRef = useRef<HTMLElement | null>(null);
  const dragInfoRef = useRef<{
    id: string;
    offsetX: number;
    offsetY: number;
    lastTargetId: string | null;
    rafId: number;
  } | null>(null);
  const gridRef = useRef<HTMLDivElement | null>(null);

  const handlePointerDown = (profileId: string) => (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("button")) return;

    const card = e.currentTarget as HTMLElement;
    const rect = card.getBoundingClientRect();

    dragInfoRef.current = {
      id: profileId,
      offsetX: e.clientX - rect.left,
      offsetY: e.clientY - rect.top,
      lastTargetId: null,
      rafId: 0,
    };
    setDragId(profileId);
    setDisplayOrder([...profiles]);

    // 防止文本选中
    document.body.style.userSelect = "none";
    document.body.style.webkitUserSelect = "none";

    // 克隆真实 DOM 节点作为幽灵卡片，保证外观完全一致
    const clone = card.cloneNode(true) as HTMLElement;
    clone.classList.add("dragGhost");
    clone.style.width = rect.width + "px";
    clone.style.height = rect.height + "px";
    clone.style.left = rect.left + "px";
    clone.style.top = rect.top + "px";
    document.body.appendChild(clone);
    dragGhostRef.current = clone;
  };

  useEffect(() => {
    if (!dragId) return;

    const onMove = (e: PointerEvent) => {
      const info = dragInfoRef.current;
      if (!info) return;
      e.preventDefault();

      // 用 RAF 节流
      if (info.rafId) return;
      info.rafId = requestAnimationFrame(() => {
        info.rafId = 0;

        // 移动幽灵
        if (dragGhostRef.current) {
          dragGhostRef.current.style.left = (e.clientX - info.offsetX) + "px";
          dragGhostRef.current.style.top = (e.clientY - info.offsetY) + "px";
        }

        // pointer-events: none 已确保 elementFromPoint 不会命中幽灵
        const el = document.elementFromPoint(e.clientX, e.clientY);

        const targetCard = (el as HTMLElement)?.closest("[data-card-id]") as HTMLElement | null;
        const targetId = targetCard?.getAttribute("data-card-id") ?? null;

        if (!targetId || targetId === info.id || targetId === info.lastTargetId) return;
        info.lastTargetId = targetId;

        // 重排 displayOrder
        setDisplayOrder(prev => {
          const fromIdx = prev.findIndex(p => p.id === info.id);
          const toIdx = prev.findIndex(p => p.id === targetId);
          if (fromIdx === -1 || toIdx === -1 || fromIdx === toIdx) return prev;
          const next = [...prev];
          const [moved] = next.splice(fromIdx, 1);
          next.splice(toIdx, 0, moved);
          return next;
        });
      });
    };

    const onUp = () => {
      const info = dragInfoRef.current;
      if (info && info.rafId) cancelAnimationFrame(info.rafId);

      // 保存最终顺序
      setDisplayOrder(prev => {
        reorderProfiles(prev);
        return prev;
      });

      dragInfoRef.current = null;
      setDragId(null);
      document.body.style.userSelect = "";
      document.body.style.webkitUserSelect = "";
      // 移除 DOM 克隆的幽灵卡片
      if (dragGhostRef.current) {
        dragGhostRef.current.remove();
        dragGhostRef.current = null;
      }
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [dragId, reorderProfiles]);

  const visibleProfiles = dragId ? displayOrder : profiles;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      {/* 状态栏 */}
      <section className="statusBar">
        <div className="statusInfo">
          <span className={`statusDot ${isRunning ? "running" : "stopped"}`} />
          <div>
            <div className="statusTitle">
              {isRunning ? `代理运行中 · ${activeName}` : "代理已停止"}
            </div>
            <div className="statusSubtitle">
              {isRunning
                ? `localhost:${activePort} → ${status?.upstream_url ?? ""}${activeModel ? ` · ${activeModel}` : ""}`
                : "选择一个代理配置并启动"}
            </div>
          </div>
        </div>
        <div className="statusMeta">
          {isRunning && <span className="statusCount">{activeRequests} 请求</span>}
          {isRunning && <button className="btn btnGhost btnSm" onClick={handleStop}>停止</button>}
        </div>
      </section>

      {error && <div className="errorBanner">{error}</div>}

      <div className="cardSection">
        <div className="cardToolbar">
          <span className="cardToolbarTitle">代理配置</span>
          <button className="btn btnPrimary btnSm" onClick={handleOpenAdd}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            添加
          </button>
        </div>

        {visibleProfiles.length > 0 ? (
          <div className="cardGrid" ref={gridRef}>
            {visibleProfiles.map((p) => {
              const isDragTarget = dragId === p.id;
              return isDragTarget ? (
                // 占位符：保持网格位置
                <div key={p.id} className="proxyCardPlaceholder" data-card-id={p.id} />
              ) : (
                <ProxyCard
                  key={p.id}
                  profile={p}
                  isActive={isRunning && activeProfileId === p.id}
                  onEdit={() => handleOpenEdit(p)}
                  onStart={() => handleStart(p)}
                  onStop={handleStop}
                  onDelete={() => handleDelete(p)}
                  onPointerDown={handlePointerDown(p.id)}
                />
              );
            })}
          </div>
        ) : (
          <div style={{ padding: "40px 20px", textAlign: "center", color: "var(--subtle)", font: "400 0.84rem/1.6 var(--font-ui)" }}>
            暂无代理配置，点击右上角「添加」按钮创建
          </div>
        )}
      </div>

      <ProxyModal open={modalOpen} profile={editProfile} onSave={handleModalSave} onClose={handleModalClose} />
    </div>
  );
}
