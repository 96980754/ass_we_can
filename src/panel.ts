import { api, type DayHistory, type Settings, type Snapshot } from "./api";
import { renderChart } from "./charts";
import {
  dayLabel,
  formatClock,
  formatDuration,
  relativeFromNow,
  todayKey,
  waitedFor,
} from "./format";

export interface PanelOptions {
  onClose: () => void;
  onChanged: (snapshot: Snapshot) => void;
  onError: (error: unknown) => void;
}

function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
}

export class Panel {
  private readonly sub = byId("panel-sub");
  private readonly banner = byId("reminder-banner");
  private readonly bannerTitle = byId("banner-title");
  private readonly bannerWait = byId("reminder-wait");
  private readonly snoozeBtn = byId<HTMLButtonElement>("snooze-btn");
  private readonly pauseBtn = byId<HTMLButtonElement>("pause-btn");
  private readonly chart = byId("chart");
  private readonly history = byId("history");
  private readonly status = byId("settings-status");

  private readonly interval = byId<HTMLInputElement>("set-interval");
  private readonly target = byId<HTMLInputElement>("set-target");
  private readonly sound = byId<HTMLInputElement>("set-sound");
  private readonly onTop = byId<HTMLInputElement>("set-ontop");
  private readonly opacity = byId<HTMLInputElement>("set-opacity");
  private readonly theme = byId<HTMLSelectElement>("set-theme");

  private snapshot: Snapshot | null = null;
  private resetArmed = 0;
  private waitTimer = 0;

  constructor(private readonly opts: PanelOptions) {
    byId("panel-close").addEventListener("click", () => this.opts.onClose());

    this.interval.addEventListener("change", () => this.patchInterval());
    this.target.addEventListener("change", () => this.patchTarget());

    this.sound.addEventListener("change", () =>
      void this.patch({ soundEnabled: this.sound.checked })
    );
    this.onTop.addEventListener("change", () =>
      void this.patch({ alwaysOnTop: this.onTop.checked })
    );
    this.theme.addEventListener("change", () =>
      void this.patch({ theme: this.theme.value as Settings["theme"] })
    );

    this.opacity.addEventListener("input", () => {
      document.documentElement.style.setProperty(
        "--ball-opacity",
        String(Number(this.opacity.value) / 100)
      );
    });
    this.opacity.addEventListener("change", () =>
      void this.patch({ opacity: Number(this.opacity.value) / 100 })
    );

    this.snoozeBtn.addEventListener("click", () => void this.snooze());
    this.pauseBtn.addEventListener("click", () => void this.togglePause());
    byId("export-btn").addEventListener("click", () => void this.exportCsv());
    byId("datadir-btn").addEventListener("click", () => void this.openDataDir());
    byId("reset-btn").addEventListener("click", () => void this.resetData());
    byId("quit-btn").addEventListener("click", () => {
      api.quit().catch(this.opts.onError);
    });

    this.startWaitClock();
  }

  get isOpen(): boolean {
    return document.getElementById("stage")?.classList.contains("expanded") ?? false;
  }

  render(snapshot: Snapshot): void {
    this.snapshot = snapshot;
    const { settings, today } = snapshot;

    this.sub.textContent = `今日 ${today.count} 次 · ${formatDuration(today.totalMs)}`;
    byId("stat-today-count").textContent = String(today.count);
    byId("stat-today-time").textContent = formatDuration(today.totalMs);
    byId("stat-streak").textContent = String(snapshot.streakDays);
    byId("stat-all-count").textContent = String(snapshot.allCount);
    byId("stat-all-time").textContent = formatDuration(snapshot.allMs);
    byId("stat-last").textContent = snapshot.lastRepMs
      ? `${relativeFromNow(snapshot.lastRepMs, snapshot.nowMs)} · ${formatClock(snapshot.lastRepMs)}`
      : "还没有记录";

    renderChart(this.chart, snapshot.last7, todayKey());

    this.interval.value = String(settings.intervalMinutes);
    this.target.value = String(settings.targetSeconds);
    this.sound.checked = settings.soundEnabled;
    this.onTop.checked = settings.alwaysOnTop;
    this.opacity.value = String(Math.round(settings.opacity * 100));
    this.theme.value = settings.theme;

    this.renderBanner();
  }

  renderBanner(): void {
    const s = this.snapshot;
    if (!s) return;
    const paused = s.settings.paused;
    const pending = !paused && s.settings.pendingSinceMs != null;

    this.banner.hidden = false;
    this.banner.classList.toggle("paused", paused);
    this.banner.classList.toggle("pending", pending);
    this.pauseBtn.textContent = paused ? "恢复提醒" : "暂停提醒";

    if (paused) {
      this.bannerTitle.textContent = "提醒已暂停";
      this.bannerWait.textContent = "恢复后重新开始计时";
      this.snoozeBtn.hidden = true;
      return;
    }

    this.snoozeBtn.hidden = !pending;
    if (pending) {
      this.bannerTitle.textContent = "该提肛了";
      this.updateWaitText();
    } else {
      this.bannerTitle.textContent = "提醒已开启";
      this.bannerWait.textContent = this.nextDueText();
    }
  }

  private nextDueText(): string {
    const due = this.snapshot?.settings.nextDueMs;
    if (!due) return "尚未开始计时";
    const minutes = Math.round((due - Date.now()) / 60_000);
    if (minutes <= 0) return "即将提醒";
    return `距下次提醒约 ${minutes} 分钟`;
  }

  async loadHistory(): Promise<void> {
    try {
      const days = await api.history(7);
      this.renderHistory(days);
    } catch (err) {
      this.opts.onError(err);
    }
  }

  showStatus(message: string, error = false): void {
    this.status.textContent = message;
    this.status.classList.toggle("error", error);
    if (!error) {
      window.setTimeout(() => {
        if (this.status.textContent === message) this.status.textContent = "";
      }, 2600);
    }
  }

  private renderHistory(days: DayHistory[]): void {
    this.history.replaceChildren();
    if (days.length === 0) {
      const empty = document.createElement("p");
      empty.className = "history-empty";
      empty.textContent = "还没有记录";
      this.history.append(empty);
      return;
    }

    const targetMs = (this.snapshot?.settings.targetSeconds ?? 5) * 1000;
    for (const day of days) {
      const details = document.createElement("details");
      details.className = "history-day";

      const summary = document.createElement("summary");
      const name = document.createElement("span");
      name.className = "day-name";
      name.textContent = `${dayLabel(day.date)} · ${day.count} 次`;
      const meta = document.createElement("span");
      meta.className = "day-meta";
      meta.textContent = formatDuration(day.totalMs);
      summary.append(name, meta);

      const list = document.createElement("div");
      list.className = "rep-list";
      for (const rep of day.reps) {
        const chip = document.createElement("span");
        chip.className = `rep-chip${rep.durationMs >= targetMs ? " hit" : ""}`;
        chip.textContent = `${formatClock(rep.startedAtMs)} · ${(rep.durationMs / 1000).toFixed(1)}s`;
        list.append(chip);
      }

      details.append(summary, list);
      this.history.append(details);
    }
  }

  private patchInterval(): void {
    const raw = Number(this.interval.value);
    if (!Number.isFinite(raw)) return;
    const value = Math.min(240, Math.max(5, Math.round(raw)));
    this.interval.value = String(value);
    void this.patch({ intervalMinutes: value }, `提醒间隔已设为 ${value} 分钟`);
  }

  private patchTarget(): void {
    const raw = Number(this.target.value);
    if (!Number.isFinite(raw)) return;
    const value = Math.min(60, Math.max(1, Math.round(raw)));
    this.target.value = String(value);
    void this.patch({ targetSeconds: value }, `每次训练时长已设为 ${value} 秒`);
  }

  private async patch(patch: Partial<Settings>, okMessage = "已保存"): Promise<void> {
    try {
      const snapshot = await api.updateSettings(patch);
      this.opts.onChanged(snapshot);
      this.showStatus(okMessage);
    } catch (err) {
      this.showStatus("保存失败", true);
      this.opts.onError(err);
    }
  }

  private async snooze(): Promise<void> {
    try {
      const snapshot = await api.snooze(5);
      this.opts.onChanged(snapshot);
      this.showStatus("5 分钟后再提醒");
    } catch (err) {
      this.opts.onError(err);
    }
  }

  private async togglePause(): Promise<void> {
    const paused = !(this.snapshot?.settings.paused ?? false);
    try {
      const snapshot = await api.setPaused(paused);
      this.opts.onChanged(snapshot);
      this.showStatus(paused ? "已暂停提醒" : "已恢复提醒");
    } catch (err) {
      this.opts.onError(err);
    }
  }

  private async exportCsv(): Promise<void> {
    try {
      const path = await api.exportCsv();
      if (path) this.showStatus(`已导出到 ${path}`);
    } catch (err) {
      this.showStatus("导出失败", true);
      this.opts.onError(err);
    }
  }

  private async openDataDir(): Promise<void> {
    try {
      await api.openDataDir();
    } catch (err) {
      this.showStatus("无法打开目录", true);
      this.opts.onError(err);
    }
  }

  private async resetData(): Promise<void> {
    const btn = byId("reset-btn");
    const now = Date.now();
    if (now - this.resetArmed > 4000) {
      this.resetArmed = now;
      btn.textContent = "再按一次确认";
      window.setTimeout(() => {
        if (Date.now() - this.resetArmed >= 4000) btn.textContent = "清空记录";
      }, 4200);
      return;
    }
    this.resetArmed = 0;
    btn.textContent = "清空记录";
    try {
      const snapshot = await api.resetData();
      this.opts.onChanged(snapshot);
      await this.loadHistory();
      this.showStatus("记录已清空");
    } catch (err) {
      this.showStatus("清空失败", true);
      this.opts.onError(err);
    }
  }

  private startWaitClock(): void {
    window.clearInterval(this.waitTimer);
    this.waitTimer = window.setInterval(() => {
      if (!this.banner.hidden) this.renderBanner();
    }, 30_000);
  }

  private updateWaitText(): void {
    const since = this.snapshot?.settings.pendingSinceMs;
    if (!since) {
      this.bannerWait.textContent = "";
      return;
    }
    this.bannerWait.textContent = waitedFor(since);
  }
}
