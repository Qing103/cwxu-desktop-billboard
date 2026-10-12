import { Button, Card } from "@fluentui/react-components";
import { Add20Regular, CalendarLtr20Regular } from "@fluentui/react-icons";
import type { Agenda, AppSettings, Snapshot } from "../../types";
import { deriveDay } from "../../dayModel";
import type { MixRow } from "../../agenda";

function AgendaRow({ row, onOpen }: { row: MixRow; onOpen: () => void }) {
  return (
    <button type="button" className={`mix-row ${row.kind}`} onClick={onOpen}>
      <div className="when"><span className="num">{row.end ? `${row.start}–${row.end}` : row.start}</span></div>
      <div className="cname"><span className="cname-main"><i className="kind-dot" aria-hidden="true" />{row.name}</span></div>
      <div className="place">{row.meta || "本机日程"}</div>
    </button>
  );
}

export function Home(props: {
  snap: Snapshot | null;
  settings: AppSettings | null;
  now: Date;
  onOpenSchedule: () => void;
  onOpenEvent: (item: Agenda | null) => void;
}) {
  const day = deriveDay(props.snap, props.settings, props.now);
  const events = day.todayMix.filter((row) => row.type === "event");
  const dateLabel = props.now.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
    weekday: "long",
  });

  return (
    <div className="stack">
      <div className="row-between">
        <h1 className="page-title">无锡学院</h1>
        <span className="hint">{dateLabel}</span>
      </div>
      <div className="about-lead">尚德笃行 · 求是鼎新</div>
      {props.settings?.displayName ? <p className="hint">欢迎回来，{props.settings.displayName}</p> : null}
      <p className="hint">桌面看板 · 教务官方入口已添加，课表同步待接入</p>

      <div className="cards">
        {[
          ["教务课表", "待接入", "从「学校服务」进入官方教务系统"],
          ["校园卡", "待接入", "校园卡余额与消费记录"],
          ["校园服务", "待接入", "宿舍、网络等服务数据"],
        ].map(([title, state, detail]) => (
          <Card key={title}>
            <div className="card-hit">
              <div className="card-k">{title}</div>
              <div className="card-v">{state}</div>
              <div className="card-u">{detail}</div>
            </div>
          </Card>
        ))}
      </div>

      <div className="row-between">
        <strong>本机日程</strong>
        <div>
          <Button appearance="subtle" icon={<Add20Regular />} onClick={() => props.onOpenEvent(null)}>添加日程</Button>
          <Button appearance="subtle" icon={<CalendarLtr20Regular />} onClick={props.onOpenSchedule}>周视图</Button>
        </div>
      </div>
      {events.length ? (
        <div className="mix">
          {events.map((row) => (
            <AgendaRow key={row.key} row={row} onOpen={() => row.event && props.onOpenEvent(row.event)} />
          ))}
        </div>
      ) : (
        <div className="empty">今天没有本机日程。添加的日程只保存在这台电脑上。</div>
      )}
      <div className="foot">无锡学院桌面看板为非官方适配版；校务数据尚未连接。</div>
    </div>
  );
}
