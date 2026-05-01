import { useState, useCallback } from "react";
import type { ProxyProfile } from "../types/app";

const STORAGE_KEY = "codex-proxy-profiles";

/** 从 localStorage 加载配置列表 */
function loadProfiles(): ProxyProfile[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed as ProxyProfile[];
  } catch {
    return [];
  }
}

/** 保存配置列表到 localStorage */
function saveProfiles(profiles: ProxyProfile[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(profiles));
}

/** 代理配置本地存储 hook */
export function useProxyStore() {
  const [profiles, setProfiles] = useState<ProxyProfile[]>(loadProfiles);

  const addProfile = useCallback((profile: Omit<ProxyProfile, "id">) => {
    const newProfile: ProxyProfile = {
      ...profile,
      id: crypto.randomUUID(),
    };
    setProfiles((prev) => {
      const next = [...prev, newProfile];
      saveProfiles(next);
      return next;
    });
  }, []);

  const updateProfile = useCallback((id: string, updates: Partial<Omit<ProxyProfile, "id">>) => {
    setProfiles((prev) => {
      const next = prev.map((p) =>
        p.id === id ? { ...p, ...updates } : p
      );
      saveProfiles(next);
      return next;
    });
  }, []);

  const deleteProfile = useCallback((id: string) => {
    setProfiles((prev) => {
      const next = prev.filter((p) => p.id !== id);
      saveProfiles(next);
      return next;
    });
  }, []);

  /** 重排配置列表（拖拽排序） */
  const reorderProfiles = useCallback((reordered: ProxyProfile[]) => {
    setProfiles(reordered);
    saveProfiles(reordered);
  }, []);

  return { profiles, addProfile, updateProfile, deleteProfile, reorderProfiles };
}
