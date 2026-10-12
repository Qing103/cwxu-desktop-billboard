import { useState } from "react";
import { Button, Field, Input } from "@fluentui/react-components";
import { api } from "../api";
import type { AppSettings } from "../types";

export function ProfileEditor(props: {
  settings: AppSettings;
  firstRun?: boolean;
  onSaved: (settings: AppSettings) => void;
}) {
  const { settings, firstRun = false, onSaved } = props;
  const [displayName, setDisplayName] = useState(settings.displayName || "");
  const [username, setUsername] = useState(settings.jwxtUsername || "");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function save(skipJwxt = false) {
    if (busy) return;
    const name = displayName.trim();
    const account = skipJwxt ? (settings.jwxtUsername || "") : username.trim();
    if (!name) {
      setError("请填写昵称。");
      return;
    }
    if (!skipJwxt && !account && password) {
      setError("填写教务密码前，请先填写教务用户名。");
      return;
    }
    if (!firstRun && settings.jwxtUsername && !account) {
      setError("如需删除已保存的账号和密码，请使用下方的删除按钮。");
      return;
    }
    if (
      account &&
      !password &&
      !(account === settings.jwxtUsername && settings.jwxtPasswordSaved)
    ) {
      setError("请输入这个教务账号的密码；密码只会保存到本机系统凭据库。");
      return;
    }
    setBusy(true);
    setError("");
    try {
      const next = await api.saveLocalProfile(name, account, skipJwxt ? null : password || null);
      setPassword("");
      setUsername(next.jwxtUsername || "");
      onSaved(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function clearCredentials() {
    if (busy || !window.confirm("删除这台电脑上保存的教务账号和密码？")) return;
    setBusy(true);
    setError("");
    try {
      const next = await api.clearLocalJwxtCredentials();
      setUsername("");
      setPassword("");
      onSaved(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={firstRun ? "login-wrap" : "stack profile-form"}>
      <div className={firstRun ? "login-card stack" : "stack"}>
        <h1 className="page-title">{firstRun ? "创建本地资料" : "用户资料与教务账号"}</h1>
        {firstRun ? (
          <p className="hint">设置昵称并保存到这台电脑。教务账号可现在填写，也可以稍后再添加。</p>
        ) : null}

        <Field label="用户昵称" required>
          <Input
            value={displayName}
            maxLength={24}
            autoComplete="nickname"
            placeholder="看板里显示的昵称"
            onChange={(_, data) => setDisplayName(data.value)}
          />
        </Field>

        <Field label="教务系统用户名">
          <Input
            value={username}
            autoComplete="username"
            placeholder="你的教务系统用户名"
            onChange={(_, data) => setUsername(data.value)}
          />
        </Field>

        <Field label="教务系统密码">
          <Input
            type="password"
            value={password}
            autoComplete="new-password"
            placeholder={settings.jwxtPasswordSaved ? "已保存；留空则继续使用" : "输入教务系统密码"}
            onChange={(_, data) => setPassword(data.value)}
          />
        </Field>

        <p className="hint">
          {settings.jwxtPasswordSaved
            ? "密码保存在本机系统凭据库，不写入普通设置文件。"
            : "保存后，密码只写入本机系统凭据库；不会从此页面发送给学校或其他服务器。"}
          {" "}当前版本尚未启用教务自动登录或课表同步。
        </p>
        {error ? <p className="hint warn" role="alert">{error}</p> : null}

        <div className="profile-actions">
          <Button appearance="primary" disabled={busy} onClick={() => save()}>
            {busy ? "正在保存…" : firstRun ? "保存并进入看板" : "保存资料"}
          </Button>
          {firstRun ? (
            <Button appearance="secondary" disabled={busy} onClick={() => save(true)}>
              暂时跳过教务账号
            </Button>
          ) : settings.jwxtUsername || settings.jwxtPasswordSaved ? (
            <Button appearance="secondary" disabled={busy} onClick={clearCredentials}>
              删除本机保存的教务账号和密码
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
