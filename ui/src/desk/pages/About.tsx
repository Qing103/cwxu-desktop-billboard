import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

export function About() {
  const [version, setVersion] = useState("");
  useEffect(() => {
    let alive = true;
    getVersion().then((v) => {
      if (alive && v.trim()) setVersion(v.trim());
    }).catch(() => undefined);
    return () => { alive = false; };
  }, []);

  return (
    <div className="stack about-page">
      <h1 className="page-title">关于看板</h1>
      <p className="about-lead">无锡学院桌面看板</p>
      <p className="hint">本项目为非官方桌面适配版，未获得学校授权或背书。</p>
      <p className="hint">当前版本不连接学校账户或数据接口。你可以将教务密码保存在本机系统凭据库；看板不会把它写入普通设置文件，也不会自动提交。</p>
      <div className="about-update">
        <div className="about-update-copy">
          {version ? <div className="about-ver">v{version}</div> : null}
          <p className="hint">自动更新未启用</p>
        </div>
      </div>
    </div>
  );
}
