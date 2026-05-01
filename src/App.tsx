import { useState, useEffect } from "react";
import { TopBar } from "./components/TopBar";
import { ProxyPanel } from "./components/ProxyPanel";
import { LogPanel } from "./components/LogPanel";
import type { Theme } from "./types/app";

export default function App() {
  const [theme, setTheme] = useState<Theme>(() => {
    const saved = localStorage.getItem("codex-proxy-theme");
    return (saved as Theme) || "dark";
  });
  const [logOpen, setLogOpen] = useState(false);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("codex-proxy-theme", theme);
  }, [theme]);

  return (
    <div className="shell">
      <div className="panel">
        <TopBar theme={theme} onThemeChange={setTheme} onOpenLogs={() => setLogOpen(true)} />
        <div className="viewStage">
          <ProxyPanel />
        </div>
      </div>
      <LogPanel open={logOpen} onClose={() => setLogOpen(false)} />
    </div>
  );
}
