import type { DayStat } from "./api";
import { dayLabel, formatDuration, weekdayLabel } from "./format";

/**
 * 近 7 天次数柱状图（单序列，无需图例）。
 * 条形用单一强调色；文字使用文本色，不用序列色。
 * 直接标注只给「今天」与「峰值」两根，其余靠悬停提示与下方历史列表（表格视图）。
 */
export function renderChart(container: HTMLElement, days: DayStat[], todayIso: string): void {
  container.replaceChildren();

  const total = days.reduce((sum, d) => sum + d.count, 0);
  if (total === 0) {
    const empty = document.createElement("div");
    empty.className = "chart-empty";
    empty.textContent = "还没有记录，按住悬浮球开始";
    container.append(empty);
    return;
  }

  const maxCount = Math.max(...days.map((d) => d.count));
  const peakDate = days.reduce((best, d) => (d.count > best.count ? d : best), days[0]);

  container.setAttribute("role", "group");
  container.setAttribute("aria-label", `近 7 天共 ${total} 次`);

  for (const day of days) {
    const isToday = day.date === todayIso;
    const isPeak = day.count === maxCount && maxCount > 0;
    const ratio = maxCount > 0 ? day.count / maxCount : 0;

    const col = document.createElement("div");
    col.className = `chart-col${isToday ? " today" : ""}`;
    col.title = `${dayLabel(day.date)} · ${day.count} 次 · ${formatDuration(day.totalMs)}`;
    col.setAttribute(
      "aria-label",
      `${dayLabel(day.date)}：${day.count} 次，${formatDuration(day.totalMs)}`
    );

    const value = document.createElement("span");
    value.className = "chart-value";
    value.textContent = isToday || (isPeak && day.date === peakDate.date) ? String(day.count) : "";

    const wrap = document.createElement("div");
    wrap.className = "chart-bar-wrap";
    const bar = document.createElement("div");
    bar.className = "chart-bar";
    bar.style.height = day.count === 0 ? "2px" : `${Math.max(6, ratio * 100)}%`;
    if (day.count === 0) bar.style.opacity = "0.16";
    wrap.append(bar);

    const label = document.createElement("span");
    label.className = "chart-label";
    label.textContent = isToday ? "今天" : weekdayLabel(day.date).replace("周", "");

    col.append(value, wrap, label);
    container.append(col);
  }
}
