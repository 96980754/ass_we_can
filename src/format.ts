const WEEKDAYS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

/** 长时长展示：12 秒 / 5 分 12 秒 / 1 小时 3 分 */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  if (total < 60) return `${total} 秒`;
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  if (minutes < 60) {
    return seconds ? `${minutes} 分 ${seconds} 秒` : `${minutes} 分`;
  }
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  const h = rest ? `${hours} 小时 ${rest} 分` : `${hours} 小时`;
  return h;
}

/** 单次训练：保留一位小数，如 5.2s */
export function formatSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)}s`;
}

export function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

export function formatClock(tsMs: number): string {
  const d = new Date(tsMs);
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

/** "2026-11-13" -> 本地时间的 Date */
export function parseLocalDate(date: string): Date {
  const [y, m, d] = date.split("-").map(Number);
  return new Date(y, (m ?? 1) - 1, d ?? 1);
}

export function todayKey(): string {
  const d = new Date();
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

/** 今天 / 昨天 / 11月12日 */
export function dayLabel(date: string): string {
  const d = parseLocalDate(date);
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const diffDays = Math.round((startOfToday - d.getTime()) / 86_400_000);
  if (diffDays === 0) return "今天";
  if (diffDays === 1) return "昨天";
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}

export function weekdayLabel(date: string): string {
  return WEEKDAYS[parseLocalDate(date).getDay()] ?? "";
}

export function shortDate(date: string): string {
  const d = parseLocalDate(date);
  return `${d.getMonth() + 1}/${d.getDate()}`;
}

/** 提醒等待时长：刚刚提醒 / 已等待 3 分钟 / 已等待 2 小时 */
export function waitedFor(tsMs: number, nowMs = Date.now()): string {
  const minutes = Math.floor(Math.max(0, nowMs - tsMs) / 60_000);
  if (minutes < 1) return "刚刚提醒";
  if (minutes < 60) return `已等待 ${minutes} 分钟`;
  return `已等待 ${Math.floor(minutes / 60)} 小时`;
}

/** 距现在多久：刚刚 / 3 分钟前 / 2 小时前 */
export function relativeFromNow(tsMs: number, nowMs = Date.now()): string {
  const diff = Math.max(0, nowMs - tsMs);
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  return `${days} 天前`;
}
