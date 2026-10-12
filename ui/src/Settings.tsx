import { useEffect, useState } from "react";
import { Button, Divider, Radio, RadioGroup, Switch } from "@fluentui/react-components";
import { Alert20Regular, Color20Regular, Person20Regular, Window20Regular } from "@fluentui/react-icons";
import type { AppSettings } from "./types";
import { ProfileEditor } from "./desk/ProfileEditor";

type Page = "hub" | "account" | "appearance" | "startup" | "panel" | "remind";

const PAGES: { id: Page; title: string; desc: string; icon: JSX.Element }[] = [
  { id: "account", title: "用户资料与教务账号", desc: "修改昵称或管理本机保存的教务账号", icon: <Person20Regular /> },
  { id: "appearance", title: "外观", desc: "浅色、深色，或跟随系统", icon: <Color20Regular /> },
  { id: "startup", title: "启动", desc: "设置开机自启", icon: <Window20Regular /> },
  { id: "panel", title: "桌面岛", desc: "自动隐藏、快捷键和悬停唤出", icon: <Window20Regular /> },
  { id: "remind", title: "日程提醒", desc: "提醒本机保存的日程", icon: <Alert20Regular /> },
];

function HotkeyField({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [capturing, setCapturing] = useState(false);
  useEffect(() => {
    if (!capturing) return;
    const held = new Set<string>();
    const label = (e: KeyboardEvent) => {
      if (e.key === "Control") return "Ctrl";
      if (e.key === "Alt") return "Alt";
      if (e.key === "Shift") return "Shift";
      if (e.key === "Meta") return "Win";
      if (e.key === "Tab") return "Tab";
      if (e.key === " ") return "Space";
      if (e.key === "Escape") return "Esc";
      if (e.key.length === 1) return e.key.toUpperCase();
      if (/^F\d{1,2}$/i.test(e.key)) return e.key.toUpperCase();
      return e.key;
    };
    const down = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && held.size === 0) {
        setCapturing(false);
        return;
      }
      held.add(label(e));
      const parts = ["Ctrl", "Alt", "Shift", "Win", "Tab"].filter((k) => held.has(k));
      for (const k of held) if (!parts.includes(k)) parts.push(k);
      const main = parts.filter((k) => !["Ctrl", "Alt", "Shift", "Win", "Tab"].includes(k));
      if (parts.length >= 2 && main.length >= 1) {
        onChange(parts.join("+"));
        setCapturing(false);
      }
    };
    const up = (e: KeyboardEvent) => held.delete(label(e));
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    return () => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
    };
  }, [capturing, onChange]);

  return (
    <Button appearance={capturing ? "primary" : "secondary"} onClick={() => setCapturing((v) => !v)}>
      {capturing ? "按下组合键…" : (value || "Ctrl+Q").replace(/\+/g, " + ")}
    </Button>
  );
}

export function SettingsPanel(props: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  onPreviewRemind: () => void;
}) {
  const { settings: s, setSettings, onPreviewRemind } = props;
  const [page, setPage] = useState<Page>("hub");
  const [motion, setMotion] = useState<"idle" | "back">("idle");
  const openPage = (next: Page) => {
    setMotion(next === "hub" && page !== "hub" ? "back" : "idle");
    setPage(next);
  };
  const set = (patch: Partial<AppSettings>) => setSettings({ ...s, ...patch });
  const title = page === "hub" ? "设置" : PAGES.find((p) => p.id === page)?.title || "设置";

  return (
    <div className={`stack settings-panel ${page === "hub" ? "settings-hub-page" : "settings-sub-page"}`}>
      <div className="row-between">
        {page === "hub" ? <h1 className="page-title">设置</h1> : (
          <div className="settings-breadcrumb">
            <button type="button" className="settings-breadcrumb-link" onClick={() => openPage("hub")}>设置</button>
            <span aria-hidden="true">›</span>
            <h1 className="page-title">{title}</h1>
          </div>
        )}
      </div>

      {page === "hub" ? (
        <div className={`set-list${motion === "back" ? " settings-hub-content" : ""}`}>
          {PAGES.map((item) => (
            <button key={item.id} type="button" className="set-item" onClick={() => openPage(item.id)}>
              <span className="set-icon" aria-hidden="true">{item.icon}</span>
              <strong>{item.title}</strong>
              <span className="chev" aria-hidden="true">›</span>
              <small>{item.desc}</small>
            </button>
          ))}
        </div>
      ) : null}

      {page === "appearance" ? (
        <div key={page} className="stack settings-sub-content">
          <p className="hint">选择看板的显示主题。</p>
          <RadioGroup value={s.theme === "light" || s.theme === "dark" ? s.theme : "system"} onChange={(_, d) => set({ theme: d.value })}>
            <Radio value="system" label="跟随系统" />
            <Radio value="light" label="浅色" />
            <Radio value="dark" label="深色" />
          </RadioGroup>
        </div>
      ) : null}

      {page === "account" ? (
        <ProfileEditor settings={s} onSaved={setSettings} />
      ) : null}

      {page === "startup" ? (
        <div key={page} className="stack settings-sub-content">
          <Switch label="开机自启" checked={!!s.autostart} onChange={(_, d) => set({ autostart: d.checked })} />
        </div>
      ) : null}

      {page === "panel" ? (
        <div key={page} className="stack settings-sub-content">
          <p className="hint">桌面岛可自动收进屏幕上沿，也可以用快捷键或鼠标悬停唤出。</p>
          <Switch label="自动隐藏" checked={!!s.autoHide} onChange={(_, d) => set({ autoHide: d.checked })} />
          {s.autoHide ? (
            <>
              <Switch label="快捷键唤出" checked={!!s.autoHideHotkey} onChange={(_, d) => set({ autoHideHotkey: d.checked })} />
              {s.autoHideHotkey ? (
                <div className="row-between">
                  <span>组合键</span>
                  <HotkeyField value={s.revealHotkey || "Ctrl+Q"} onChange={(revealHotkey) => set({ revealHotkey })} />
                </div>
              ) : null}
              <Switch label="悬停唤出" checked={!!s.autoHideHover} onChange={(_, d) => set({ autoHideHover: d.checked })} />
            </>
          ) : <p className="hint">打开窗口后，桌面岛会在无人使用时收起。</p>}
        </div>
      ) : null}

      {page === "remind" ? (
        <div key={page} className="stack settings-sub-content">
          <p className="hint">添加本机日程时开启提醒，桌面岛会在开始前提示。提醒数据保存在这台电脑上。</p>
          <Divider />
          <Button appearance="secondary" onClick={onPreviewRemind}>预览提醒动效</Button>
        </div>
      ) : null}
    </div>
  );
}
