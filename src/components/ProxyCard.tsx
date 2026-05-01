import type { ProxyProfile } from "../types/app";

interface ProxyCardProps {
  profile: ProxyProfile;
  isActive: boolean;
  onEdit: () => void;
  onStart: () => void;
  onStop: () => void;
  onDelete: () => void;
  onPointerDown: (e: React.PointerEvent) => void;
}

export function ProxyCard({ profile, isActive, onEdit, onStart, onStop, onDelete, onPointerDown }: ProxyCardProps) {
  const truncatedUrl = profile.baseUrl.length > 36
    ? profile.baseUrl.slice(0, 36) + "..."
    : profile.baseUrl;

  return (
    <div
      className={`proxyCard ${isActive ? "active" : ""}`}
      data-card-id={profile.id}
      onPointerDown={onPointerDown}
    >
      <div className="proxyCardHeader">
        <div className="proxyCardName">
          <span className={`statusDot ${isActive ? "running" : "stopped"}`} />
          {profile.name}
        </div>
        <button
          className="btn btnGhost btnSm"
          onClick={onDelete}
          aria-label="删除"
          style={{ visibility: isActive ? "hidden" : "visible" }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3 6 5 6 21 6" />
            <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
            <path d="M10 11v6" />
            <path d="M14 11v6" />
          </svg>
        </button>
      </div>

      <div className="proxyCardUrl" title={profile.baseUrl}>{truncatedUrl}</div>
      <div className="proxyCardModel">
        {profile.model}
        {" · "}
        端口 {profile.port}
      </div>

      <div className="proxyCardFooter">
        <div className="proxyCardActions">
          <button
            className="btn btnGhost btnSm"
            onClick={onEdit}
            style={{ visibility: isActive ? "hidden" : "visible" }}
          >
            编辑
          </button>
        </div>
        {isActive ? (
          <button className="btn btnGhost btnSm" onClick={onStop}>停止</button>
        ) : (
          <button className="btn btnPrimary btnSm" onClick={onStart}>启动</button>
        )}
      </div>
    </div>
  );
}
