import { useEffect, useRef, useState } from "react";
import { Button, Popover, PopoverSurface, PopoverTrigger } from "@fluentui/react-components";
import { Add20Regular } from "@fluentui/react-icons";
import { agendasOnDate, repeatText, whenText } from "../../agenda";
import { WEEKDAY_SHORT, addDays, coursesOnDay, datesOfTeachingWeek, mondayOf } from "../../course";
import { placeLabel, ymd } from "../../format";
import type { Agenda, AppSettings, Snapshot } from "../../types";
import { deriveDay } from "../../dayModel";
import {
  AXIS_END_MIN,
  AXIS_SPAN_MIN,
  AXIS_START_MIN,
  agendaDayMinutes,
  courseLane,
  layoutDay,
  sectionText,
  weeksText,
  type LaneItem,
} from "../../week";

const HOURS = Array.from({ length: 15 }, (_, i) => i + 8);

function EventFacts({ event }: { event: Agenda }) {
  const again = repeatText(event);
  return (
    <div className="pop">
      <p className="pop-time num">{whenText(event)}</p>
      <p>{event.name}</p>
      {again ? <p>{again}</p> : null}
      {event.location ? <p>{event.location}</p> : null}
      {event.note ? <p>{event.note}</p> : null}
    </div>
  );
}

export function Schedule(props: {
  snap: Snapshot | null;
  settings: AppSettings | null;
  now: Date;
  onOpenEvent: (item: Agenda | null) => void;
}) {
  const { snap, settings, now } = props;
  const day = deriveDay(snap, settings, now);
  const [semWeek, setSemWeek] = useState(1);
  const [calMonday, setCalMonday] = useState(() => mondayOf(now));
  const seeded = useRef(false);
  const [openKey, setOpenKey] = useState<string | null>(null);

  useEffect(() => {
    if (seeded.current || !day.weekNow) return;
    seeded.current = true;
    setSemWeek(Math.min(Math.max(1, day.weekNow), day.semMax));
  }, [day.weekNow, day.semMax]);

  const teaching = false;
  const dates = teaching
    ? datesOfTeachingWeek(day.week1, semWeek)
    : Array.from({ length: 7 }, (_, i) => addDays(calMonday, i));
  const nowMin = now.getHours() * 60 + now.getMinutes();
  const today = ymd(now);

  return (
    <div className="stack">
      <div className="row-between">
        <h1 className="page-title">日程</h1>
        <div className="week-toolbar">
          {teaching ? (
            <>
              <Button appearance="subtle" disabled={semWeek <= 1} onClick={() => setSemWeek((w) => Math.max(1, w - 1))}>上一周</Button>
              <strong>第 {semWeek} 周</strong>
              <Button appearance="subtle" disabled={semWeek >= day.semMax} onClick={() => setSemWeek((w) => Math.min(day.semMax, w + 1))}>下一周</Button>
            </>
          ) : (
            <>
              <Button appearance="subtle" onClick={() => setCalMonday((d) => addDays(d, -7))}>上一周</Button>
              <strong>{calMonday.getMonth() + 1}/{calMonday.getDate()} 起</strong>
              <Button appearance="subtle" onClick={() => setCalMonday((d) => addDays(d, 7))}>下一周</Button>
            </>
          )}
          <Button appearance="primary" icon={<Add20Regular />} onClick={() => props.onOpenEvent(null)}>添加日程</Button>
        </div>
      </div>

      <p className="hint">无锡学院教务数据尚未接入；此处显示和编辑保存在本机的日程。</p>
      {!dates.length ? (
        <div className="empty">本周没有本机日程。</div>
      ) : (
        <div className="week-scroll">
          <div className="week-head">
            <div />
            {dates.map((d, i) => {
              const key = ymd(d);
              return (
                <div key={key} className={key === today ? "today" : undefined}>
                  周{WEEKDAY_SHORT[i]}
                  <small>{d.getMonth() + 1}/{d.getDate()}</small>
                </div>
              );
            })}
          </div>
          <div className="week-body">
            <div className="week-hours">
              {HOURS.map((h) => (
                <div key={h} className="hour-label" style={{ top: `${((h * 60 - AXIS_START_MIN) / AXIS_SPAN_MIN) * 100}%` }}>
                  {String(h).padStart(2, "0")}:00
                </div>
              ))}
            </div>
            {dates.map((d, i) => {
              const key = ymd(d);
              const dow = i + 1;
              const courses = snap?.loggedIn && snap.semester && teaching ? coursesOnDay(snap.semester, semWeek, dow) : [];
              const items: LaneItem[] = courses.map((c) => courseLane(c));
              for (const a of agendasOnDate(snap?.agenda, key)) {
                const span = agendaDayMinutes(a, key);
                if (!span) continue;
                items.push({
                  key: `e-${a.id}-${key}`,
                  kind: "event",
                  name: a.name,
                  startMin: span.startMin,
                  endMin: span.endMin,
                  event: a,
                });
              }
              const layout = layoutDay(items);
              const showNow = key === today && nowMin >= AXIS_START_MIN && nowMin <= AXIS_END_MIN;
              return (
                <div className="week-col" key={key}>
                  <div className="week-axis">
                    {HOURS.map((h) => (
                      <div key={h} className="hour-line" style={{ top: `${((h * 60 - AXIS_START_MIN) / AXIS_SPAN_MIN) * 100}%` }} />
                    ))}
                    {showNow ? <div className="now-line" style={{ top: `${((nowMin - AXIS_START_MIN) / AXIS_SPAN_MIN) * 100}%` }} /> : null}
                    {layout.placed.map((b) => {
                      const style = {
                        top: `${b.top * 100}%`,
                        height: `${b.height * 100}%`,
                        left: `calc(${(b.column / b.columns) * 100}% + 2px)`,
                        width: `calc(${100 / b.columns}% - 4px)`,
                      };
                      if (b.kind === "course" && b.course) {
                        const c = b.course;
                        const sec = sectionText(c.sections);
                        return (
                          <Popover
                            key={b.key}
                            open={openKey === b.key}
                            onOpenChange={(_, data) => setOpenKey(data.open ? b.key : null)}
                            positioning="after"
                          >
                            <PopoverTrigger disableButtonEnhancement>
                              <button
                                type="button"
                                className="block course"
                                style={style}
                                onMouseEnter={() => setOpenKey(b.key)}
                                onMouseLeave={() => setOpenKey(null)}
                              >
                                <span className="bn">{c.name}</span>
                                <span className="bm">{placeLabel(c.place)}</span>
                                {sec ? <span className="bm">{sec}</span> : null}
                              </button>
                            </PopoverTrigger>
                            <PopoverSurface>
                              <div className="pop">
                                <p className="pop-time num">{c.start}–{c.end}</p>
                                <p>{c.name}</p>
                                <p>{c.teacher?.trim() || "教师未填"}</p>
                                <p>{placeLabel(c.place)}</p>
                                <p>{weeksText(c.weeks)}</p>
                              </div>
                            </PopoverSurface>
                          </Popover>
                        );
                      }
                      return (
                        <Popover
                          key={b.key}
                          open={openKey === b.key}
                          onOpenChange={(_, data) => setOpenKey(data.open ? b.key : null)}
                          positioning="above"
                        >
                          <PopoverTrigger disableButtonEnhancement>
                            <button
                              type="button"
                              className="block event"
                              style={style}
                              onMouseEnter={() => setOpenKey(b.key)}
                              onMouseLeave={() => setOpenKey(null)}
                              onClick={() => b.event && props.onOpenEvent(b.event)}
                            >
                              <span className="bn">{b.name}</span>
                              {b.event?.location?.trim() ? <span className="bm">{b.event.location.trim()}</span> : null}
                            </button>
                          </PopoverTrigger>
                          <PopoverSurface>
                            {b.event ? <EventFacts event={b.event} /> : null}
                          </PopoverSurface>
                        </Popover>
                      );
                    })}
                  </div>
                  {layout.outside.length ? (
                    <div className="week-out">
                      <div className="cap">轴外</div>
                      {layout.outside.map((item) => (
                        <button key={item.key} type="button" onClick={() => item.event && props.onOpenEvent(item.event)}>
                          {item.name}
                          {item.startMin >= 0 ? ` ${String(Math.floor(item.startMin / 60)).padStart(2, "0")}:${String(item.startMin % 60).padStart(2, "0")}` : ""}
                        </button>
                      ))}
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
      )}
      <div className="foot">日程存在这台电脑上。课表数据待接入。</div>
    </div>
  );
}
