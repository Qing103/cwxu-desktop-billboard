import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { FluentProvider, NavDrawer, NavDrawerBody, NavItem } from "@fluentui/react-components";
import {
  CalendarLtr20Regular,
  DoorArrowRight20Regular,
  Home20Regular,
  Info20Regular,
  Settings20Regular,
} from "@fluentui/react-icons";
import { api } from "../api";
import { EventForm } from "../EventForm";
import { resolveTheme, withSettingDefaults } from "../prefs";
import type { Agenda, AppSettings, Snapshot } from "../types";
import { useClock } from "../useClock";
import { nidongDarkTheme, nidongLightTheme } from "./theme";
import { About } from "./pages/About";
import { Doors } from "./pages/Doors";
import { Home } from "./pages/Home";
import { Schedule } from "./pages/Schedule";
import { SettingsPage } from "./pages/Settings";
import { ProfileEditor } from "./ProfileEditor";

type Page = "home" | "schedule" | "doors" | "settings" | "about";

const NAV: { value: Page; label: string; icon: JSX.Element }[] = [
  { value: "home", label: "看板", icon: <Home20Regular /> },
  { value: "schedule", label: "本机日程", icon: <CalendarLtr20Regular /> },
  { value: "doors", label: "学校服务", icon: <DoorArrowRight20Regular /> },
  { value: "settings", label: "设置", icon: <Settings20Regular /> },
  { value: "about", label: "关于", icon: <Info20Regular /> },
];

export function DeskApp() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [page, setPage] = useState<Page>("home");
  const [eventOpen, setEventOpen] = useState(false);
  const [editing, setEditing] = useState<Agenda | null>(null);
  const [eventNonce, setEventNonce] = useState(0);
  const [eventErr, setEventErr] = useState("");
  const [eventBusy, setEventBusy] = useState(false);
  const hydrated = useRef(false);

  const { now } = useClock(page === "schedule");
  const mode = resolveTheme(settings?.theme);

  useEffect(() => {
    let alive = true;
    Promise.all([api.bootstrap(), api.settings()]).then(([initialSnap, initialSettings]) => {
      if (!alive) return;
      setSnap(initialSnap);
      setSettings(withSettingDefaults(initialSettings));
      hydrated.current = true;
    }).catch(() => undefined);
    return () => { alive = false; };
  }, []);

  useEffect(() => {
    if (!settings || !hydrated.current) return;
    const timer = window.setTimeout(() => {
      api.saveSettings(settings).catch(() => undefined);
    }, 300);
    return () => window.clearTimeout(timer);
  }, [settings]);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", mode);
  }, [mode]);

  useEffect(() => {
    if (settings?.theme === "light" || settings?.theme === "dark") return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const onChange = () => document.documentElement.setAttribute("data-theme", resolveTheme("system"));
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [settings?.theme]);

  useEffect(() => {
    const unsubs: Array<() => void> = [];
    listen<Snapshot>("snapshot-updated", (e) => setSnap(e.payload)).then((f) => unsubs.push(f));
    listen<AppSettings>("settings-updated", (e) => {
      const next = withSettingDefaults(e.payload);
      setSettings((current) => current && JSON.stringify(current) === JSON.stringify(next) ? current : next);
    }).then((f) => unsubs.push(f));
    return () => unsubs.forEach((f) => f());
  }, []);

  function openEvent(item: Agenda | null) {
    setEditing(item);
    setEventErr("");
    setEventNonce((n) => n + 1);
    setEventOpen(true);
  }

  async function saveEvent(item: Agenda) {
    if (eventBusy) return;
    setEventBusy(true);
    setEventErr("");
    try {
      setSnap(await api.saveAgenda(item));
      setEventOpen(false);
    } catch (e) {
      setEventErr(String(e));
    } finally {
      setEventBusy(false);
    }
  }

  async function deleteEvent(id: string) {
    if (eventBusy) return;
    setEventBusy(true);
    setEventErr("");
    try {
      setSnap(await api.deleteAgenda(id));
      setEventOpen(false);
    } catch (e) {
      setEventErr(String(e));
    } finally {
      setEventBusy(false);
    }
  }

  const theme = mode === "light" ? nidongLightTheme : nidongDarkTheme;
  return (
    <FluentProvider theme={theme} style={{ height: "100%" }}>
      <div className="desk-root">
        {!snap || !settings ? (
          <div className="desk-main desk-pending" role="status">
            <i className="status-spin" aria-hidden="true" />
            <span>正在打开…</span>
          </div>
        ) : settings.profileSetupComplete !== true ? (
          <main className="desk-main">
            <div className="page-enter">
              <ProfileEditor
                settings={settings}
                firstRun
                onSaved={(next) => setSettings(withSettingDefaults(next))}
              />
            </div>
          </main>
        ) : (
          <div className="desk-shell">
            <NavDrawer
              className="desk-nav"
              open
              type="inline"
              selectedValue={page}
              onNavItemSelect={(_, data) => {
                const value = String(data.value || "");
                if (NAV.some((item) => item.value === value)) setPage(value as Page);
              }}
            >
              <NavDrawerBody>
                {NAV.map((item) => (
                  <NavItem key={item.value} value={item.value} icon={item.icon}>{item.label}</NavItem>
                ))}
              </NavDrawerBody>
            </NavDrawer>
            <main className="desk-main">
              <div key={page} className="page-enter">
                {page === "home" && settings ? (
                  <Home snap={snap} settings={settings} now={now} onOpenSchedule={() => setPage("schedule")} onOpenEvent={openEvent} />
                ) : null}
                {page === "schedule" ? <Schedule snap={snap} settings={settings} now={now} onOpenEvent={openEvent} /> : null}
                {page === "doors" ? <Doors /> : null}
                {page === "settings" && settings ? (
                  <SettingsPage settings={settings} setSettings={setSettings} onPreviewRemind={() => { api.previewRemind().catch(() => undefined); }} />
                ) : null}
                {page === "about" ? <About /> : null}
              </div>
            </main>
          </div>
        )}
        <EventForm
          key={eventNonce}
          open={eventOpen}
          initial={editing}
          now={now}
          busy={eventBusy}
          err={eventErr}
          onClose={() => setEventOpen(false)}
          onSave={saveEvent}
          onDelete={editing?.id ? deleteEvent : undefined}
        />
      </div>
    </FluentProvider>
  );
}
