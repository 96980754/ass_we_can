import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { api, type Snapshot } from "./api";
import { Ball } from "./ball";
import { Onboarding } from "./onboarding";
import { Panel } from "./panel";

const stage = document.getElementById("stage")!;
const ballEl = document.getElementById("ball")!;

let snapshot: Snapshot | null = null;
let panelOpen = false;
let busy = false;

function reportError(error: unknown): void {
  console.error(error);
  if (panelOpen) panel.showStatus(String(error ?? "操作失败"), true);
}

function apply(s: Snapshot): void {
  snapshot = s;

  document.documentElement.dataset.theme = s.settings.theme;
  document.documentElement.style.setProperty(
    "--ball-opacity",
    String(s.settings.opacity)
  );

  ball.setTargetSeconds(s.settings.targetSeconds);
  ball.setSession(s.sessionReps, s.settings.repsPerSession);
  ball.setTodayCount(s.today.count);
  ball.setReminderPending(!s.settings.paused && s.settings.pendingSinceMs != null);
  panel.render(s);
}

const onboarding = new Onboarding();

const ball = new Ball(ballEl, {
  getTargetMs: () => (snapshot?.settings.targetSeconds ?? 5) * 1000,
  getSoundEnabled: () => snapshot?.settings.soundEnabled ?? true,
  isBlocked: () => onboarding.isOpen,
  isPreCheckEnabled: () => snapshot?.settings.preCheckEnabled ?? true,
  onTap: () => void togglePanel(),
  onDragStart: async () => {
    try {
      if (panelOpen) await togglePanel(false);
      await api.startDrag();
    } catch (err) {
      reportError(err);
    }
  },
  onRecord: (startedAtMs, durationMs) => {
    const prevDone = snapshot?.sessionReps ?? 0;
    const target = snapshot?.settings.repsPerSession ?? 10;
    api
      .recordRep(startedAtMs, durationMs)
      .then((s) => {
        const completed = s.sessionReps === 0 && prevDone + 1 >= target;
        apply(s);
        if (completed) ball.celebrate(target);
        if (panelOpen) void panel.loadHistory();
      })
      .catch(reportError);
  },
  onError: reportError,
});

const panel = new Panel({
  onClose: () => void togglePanel(false),
  onChanged: (s) => apply(s),
  onTutorial: () => startTutorial(false),
  onError: reportError,
});

async function togglePanel(force?: boolean): Promise<void> {
  const next = force ?? !panelOpen;
  // 首次提示 / 教程未完成时不允许收起面板
  if (!next && onboarding.isOpen) return;
  if (next === panelOpen || busy) return;
  busy = true;
  try {
    await api.setPanelOpen(next);
    panelOpen = next;
    stage.classList.toggle("expanded", next);
    stage.classList.toggle("compact", !next);
    stage.dataset.panel = next ? "open" : "closed";
    if (next) {
      panel.renderBanner();
      await panel.loadHistory();
    }
  } catch (err) {
    reportError(err);
  } finally {
    busy = false;
  }
}

function startTutorial(firstRun: boolean): void {
  void togglePanel(true);
  onboarding.showTutorial(async () => {
    try {
      const s = await api.updateSettings({ tutorialDone: true });
      apply(s);
    } catch (err) {
      reportError(err);
    }
    if (!firstRun) panel.showStatus("教程已结束，可随时在设置里重看");
  });
}

async function refresh(): Promise<void> {
  try {
    const s = await api.snapshot();
    apply(s);
    if (panelOpen) await panel.loadHistory();
  } catch (err) {
    reportError(err);
  }
}

async function ensureNotificationPermission(): Promise<void> {
  try {
    if (!(await isPermissionGranted())) {
      await requestPermission();
    }
  } catch {
    // 系统通知不可用时，提醒会降级为球体脉冲 + 应用内提示
  }
}

async function boot(): Promise<void> {
  // 基础防护：本应用不需要浏览器默认的右键菜单与拖拽行为
  document.addEventListener(
    "pointerdown",
    (e) => {
      const t = e.target as HTMLElement;
      document.title = `pd:${t?.tagName ?? "?"}#${t?.id ?? ""}@${Math.round(
        e.clientX
      )},${Math.round(e.clientY)}`;
    },
    true
  );

  window.addEventListener("contextmenu", (e) => e.preventDefault());
  window.addEventListener("dragover", (e) => e.preventDefault());
  window.addEventListener("drop", (e) => e.preventDefault());

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && panelOpen) void togglePanel(false);
  });

  stage.classList.add("compact");
  stage.classList.remove("expanded");

  try {
    apply(await api.snapshot());
  } catch (err) {
    reportError(err);
  }

  // 开发热重载 / 异常重载后，窗口尺寸可能与前端状态不一致，先归位为收起态
  try {
    await api.setPanelOpen(false);
  } catch {
    // 忽略：部分平台不支持程序化调整尺寸
  }
  panelOpen = false;
  stage.classList.add("compact");
  stage.classList.remove("expanded");

  // 首次启动：强制提示「不要憋气」，随后引导新手教程
  if (snapshot && !snapshot.settings.breathNoticeSeen) {
    await togglePanel(true);
    onboarding.showBreathNotice(async () => {
      try {
        const s = await api.updateSettings({ breathNoticeSeen: true });
        apply(s);
        if (!s.settings.tutorialDone) startTutorial(true);
      } catch (err) {
        reportError(err);
      }
    });
  }

  await listen("reminder:due", () => {
    ball.pingReminder(snapshot?.settings.soundEnabled ?? true);
    void refresh();
  });

  await listen<Snapshot>("state:changed", (event) => {
    apply(event.payload);
  });

  await listen("panel:open", () => {
    void togglePanel(true);
  });

  await listen("panel:close", () => {
    void togglePanel(false);
  });

  void ensureNotificationPermission();

  // 每分钟刷新一次「相对时间」等派生文案
  window.setInterval(() => {
    if (snapshot && panelOpen) panel.render(snapshot);
  }, 60_000);
}

void boot();
