import { useRef, useState } from "react";
import { Building20Regular } from "@fluentui/react-icons";
import { api } from "../../api";

export function Doors() {
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);

  async function openOfficialSite(url: string) {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setErr("");
    try {
      await api.openDoor(url, "direct");
    } catch (e) {
      setErr(String(e).replace(/^Error: /, ""));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }

  return (
    <div className="stack">
      <div>
        <h1 className="page-title">学校服务</h1>
        <p className="hint">教务自动登录和课表同步尚未启用。当前可在学校官方页面登录；如在看板保存了密码，它只存于本机系统凭据库，尚不会自动提交。</p>
      </div>
      {err ? <p className="hint warn">{err}</p> : null}
      <div className="door-kicker">无锡学院官方入口</div>
      <div className="door-list">
        <button type="button" className={busy ? "door-row opening" : "door-row"} disabled={busy} onClick={() => { void openOfficialSite("https://www.cwxu.edu.cn/"); }}>
          <span className="door-icon" aria-hidden="true"><Building20Regular /></span>
          <span className="door-name">无锡学院官方网站</span>
          <span className="door-tag">{busy ? "正在打开" : "cwxu.edu.cn"}</span>
        </button>
        <button type="button" className={busy ? "door-row opening" : "door-row"} disabled={busy} onClick={() => { void openOfficialSite("https://jwgl.cwxu.edu.cn/jwglxt/xtgl/login_slogin.htm"); }}>
          <span className="door-icon" aria-hidden="true"><Building20Regular /></span>
          <span className="door-name">教务系统 · 学生入口</span>
          <span className="door-tag">{busy ? "正在打开" : "jwgl.cwxu.edu.cn"}</span>
        </button>
        <button type="button" className={busy ? "door-row opening" : "door-row"} disabled={busy} onClick={() => { void openOfficialSite("https://jwgl.cwxu.edu.cn/jwglxt/kbcx/xskbcx_cxXsgrkb.html?gnmkdm=N2151"); }}>
          <span className="door-icon" aria-hidden="true"><Building20Regular /></span>
          <span className="door-name">个人课表查询</span>
          <span className="door-tag">{busy ? "正在打开" : "教务系统"}</span>
        </button>
      </div>
    </div>
  );
}
