import { SettingsPanel } from "../../Settings";
import type { AppSettings } from "../../types";

export function SettingsPage(props: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  onPreviewRemind: () => void;
}) {
  return <SettingsPanel {...props} />;
}