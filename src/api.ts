import { invoke } from "@tauri-apps/api/core";

export interface Settings {
  intervalMinutes: number;
  targetSeconds: number;
  repsPerSession: number;
  sessionReps: number;
  minValidMs: number;
  soundEnabled: boolean;
  opacity: number;
  alwaysOnTop: boolean;
  theme: "auto" | "light" | "dark";
  breathNoticeSeen: boolean;
  tutorialDone: boolean;
  preCheckEnabled: boolean;
  paused: boolean;
  nextDueMs: number | null;
  pendingSinceMs: number | null;
  ball: { x: number | null; y: number | null };
}

export interface DayStat {
  date: string;
  count: number;
  totalMs: number;
}

export interface Rep {
  id: string;
  startedAtMs: number;
  durationMs: number;
  localDate: string;
  tzOffsetMin: number;
}

export interface DayHistory {
  date: string;
  count: number;
  totalMs: number;
  reps: Rep[];
}

export interface Snapshot {
  settings: Settings;
  sessionReps: number;
  today: DayStat;
  allCount: number;
  allMs: number;
  streakDays: number;
  last7: DayStat[];
  lastRepMs: number | null;
  dataDir: string;
  nowMs: number;
}

export function invokeSafe<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

export const api = {
  snapshot: () => invokeSafe<Snapshot>("get_snapshot"),

  recordRep: (startedAtMs: number, durationMs: number) =>
    invokeSafe<Snapshot>("record_rep", { startedAtMs, durationMs }),

  history: (days = 7) => invokeSafe<DayHistory[]>("get_history", { days }),

  updateSettings: (patch: Partial<Settings>) =>
    invokeSafe<Snapshot>("update_settings", { patch }),

  setPaused: (paused: boolean) => invokeSafe<Snapshot>("set_paused", { paused }),

  snooze: (minutes: number) => invokeSafe<Snapshot>("snooze", { minutes }),

  setInRep: (inRep: boolean) => invokeSafe<void>("set_in_rep", { inRep }),

  setPanelOpen: (open: boolean) => invokeSafe<void>("set_panel_open", { open }),

  startDrag: () => invokeSafe<void>("start_drag"),

  exportCsv: () => invokeSafe<string | null>("export_csv"),

  openDataDir: () => invokeSafe<void>("open_data_dir"),

  resetData: () => invokeSafe<Snapshot>("reset_data"),

  quit: () => invokeSafe<void>("quit_app"),

  setVisible: (visible: boolean) => invokeSafe<void>("set_ball_visible", { visible }),
};
