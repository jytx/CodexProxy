import { useState, useEffect } from "react";
import type { ProxyProfile } from "../types/app";

interface ProxyModalProps {
  open: boolean;
  profile?: ProxyProfile;
  onSave: (data: Omit<ProxyProfile, "id">) => void;
  onClose: () => void;
}

const EMPTY_FORM: Omit<ProxyProfile, "id"> = {
  name: "",
  baseUrl: "",
  apiKey: "",
  model: "",
  port: 9090,
};

/** 添加/编辑代理配置弹窗 */
export function ProxyModal({ open, profile, onSave, onClose }: ProxyModalProps) {
  const [form, setForm] = useState(EMPTY_FORM);
  const [showApiKey, setShowApiKey] = useState(false);

  const isEdit = !!profile;

  useEffect(() => {
    if (open) {
      setForm(profile ? {
        name: profile.name,
        baseUrl: profile.baseUrl,
        apiKey: profile.apiKey,
        model: profile.model,
        port: profile.port,
      } : EMPTY_FORM);
      setShowApiKey(false);
    }
  }, [open, profile]);

  const updateField = (key: keyof typeof form, value: string | number) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const handleSubmit = () => {
    if (!form.name.trim() || !form.baseUrl.trim() || !form.apiKey.trim() || !form.model.trim()) {
      return;
    }
    onSave(form);
  };

  const handleReset = () => {
    setForm(EMPTY_FORM);
    setShowApiKey(false);
  };

  if (!open) return null;

  const isValid = form.name.trim() && form.baseUrl.trim() && form.apiKey.trim() && form.model.trim();

  return (
    <div className="modalOverlay" onClick={onClose}>
      <div className="modalContent" onClick={(e) => e.stopPropagation()}>
        <div className="modalHeader">
          <span className="modalTitle">{isEdit ? "编辑代理配置" : "添加代理配置"}</span>
          <button className="btnIcon" onClick={onClose} aria-label="关闭">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <div className="modalBody">
          <label className="label" htmlFor="modal-name">名称 *</label>
          <input
            id="modal-name"
            className="inputField"
            type="text"
            placeholder="例如：小米 MiMo"
            value={form.name}
            onChange={(e) => updateField("name", e.target.value)}
          />

          <label className="label" htmlFor="modal-baseurl">Base URL *</label>
          <input
            id="modal-baseurl"
            className="inputField"
            type="text"
            placeholder="https://api.minimaxi.com/v1"
            value={form.baseUrl}
            onChange={(e) => updateField("baseUrl", e.target.value)}
          />

          <label className="label" htmlFor="modal-apikey">API Key *</label>
          <div className="modalFieldGroup">
            <input
              id="modal-apikey"
              className="inputField"
              type={showApiKey ? "text" : "password"}
              placeholder="输入你的 API Key"
              value={form.apiKey}
              onChange={(e) => updateField("apiKey", e.target.value)}
            />
            <button
              type="button"
              className="eyeToggle"
              onClick={() => setShowApiKey((v) => !v)}
              aria-label={showApiKey ? "隐藏 API Key" : "显示 API Key"}
            >
              {showApiKey ? (
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
                  <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
                  <line x1="1" y1="1" x2="23" y2="23" />
                </svg>
              ) : (
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                  <circle cx="12" cy="12" r="3" />
                </svg>
              )}
            </button>
          </div>

          <label className="label" htmlFor="modal-model">模型名称 *</label>
          <input
            id="modal-model"
            className="inputField"
            type="text"
            placeholder="mimo-v2.5-pro"
            value={form.model}
            onChange={(e) => updateField("model", e.target.value)}
          />

          <label className="label" htmlFor="modal-port">监听端口 *</label>
          <input
            id="modal-port"
            className="inputField"
            type="number"
            value={form.port}
            onChange={(e) => updateField("port", parseInt(e.target.value, 10) || 9090)}
          />
        </div>

        <div className="modalFooter">
          <button className="btn btnGhost" onClick={handleReset}>重置</button>
          <button
            className="btn btnPrimary"
            disabled={!isValid}
            onClick={handleSubmit}
          >
            {isEdit ? "保存" : "添加"}
          </button>
        </div>
      </div>
    </div>
  );
}
