import { api } from "./api";
import { formatSeconds } from "./format";
import { playTone, primeAudio } from "./audio";

const TAP_MS = 250;
const DRAG_PX = 6;
const MIN_VALID_MS = 1000;
const RING_CIRCUMFERENCE = 2 * Math.PI * 45;

type Phase = "idle" | "pending" | "dragging" | "feedback";

export interface BallOptions {
  getTargetMs: () => number;
  getSoundEnabled: () => boolean;
  onTap: () => void;
  onRecord: (startedAtMs: number, durationMs: number) => void;
  onDragStart: () => void;
  onError: (error: unknown) => void;
}

export class Ball {
  private readonly el: HTMLElement;
  private readonly ringFill: SVGCircleElement;
  private readonly readout: HTMLElement;
  private readonly hint: HTMLElement;
  private readonly badge: HTMLElement;
  private readonly alert: HTMLElement;

  private phase: Phase = "idle";
  private locked = false;
  private downPerf = 0;
  private downWall = 0;
  private downX = 0;
  private downY = 0;
  private raf = 0;
  private reached = false;
  private feedbackTimer = 0;
  private dragWatchdog = 0;
  private targetMs = 5000;

  constructor(el: HTMLElement, private readonly opts: BallOptions) {
    this.el = el;
    this.ringFill = el.querySelector<SVGCircleElement>("#ring-fill")!;
    this.readout = el.querySelector<HTMLElement>("#ball-readout")!;
    this.hint = el.querySelector<HTMLElement>("#ball-hint")!;
    this.badge = el.querySelector<HTMLElement>("#ball-badge")!;
    this.alert = el.querySelector<HTMLElement>("#ball-alert")!;

    this.ringFill.style.strokeDasharray = String(RING_CIRCUMFERENCE);
    this.ringFill.style.strokeDashoffset = String(RING_CIRCUMFERENCE);

    el.addEventListener("pointerdown", this.onPointerDown);
    el.addEventListener("pointermove", this.onPointerMove);
    el.addEventListener("pointerup", this.onPointerUp);
    el.addEventListener("pointercancel", this.onPointerCancel);
    el.addEventListener("lostpointercapture", this.onLostCapture);
    el.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      this.opts.onTap();
    });
    el.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        this.opts.onTap();
      }
    });
    window.addEventListener("blur", this.onWindowBlur);
  }

  setTargetSeconds(seconds: number): void {
    this.targetMs = Math.max(1, seconds) * 1000;
  }

  setTodayCount(count: number): void {
    this.badge.textContent = String(count);
  }

  setReminderPending(pending: boolean): void {
    this.alert.hidden = !pending;
    this.el.dataset.reminder = pending ? "1" : "0";
  }

  /** 提醒到点：脉冲发光 + 提示音（不震动，震动只用于长按）。 */
  pingReminder(soundEnabled: boolean): void {
    this.el.classList.remove("pulse");
    // 强制重排以重启动画
    void this.el.offsetWidth;
    this.el.classList.add("pulse");
    window.setTimeout(() => this.el.classList.remove("pulse"), 4200);
    if (soundEnabled) playTone("remind");
  }

  resetVisual(): void {
    this.stopFrame();
    this.locked = false;
    this.reached = false;
    this.el.classList.remove("rumble", "strong", "shake", "success", "reached");
    this.readout.hidden = true;
    this.hint.hidden = false;
    this.ringFill.style.strokeDashoffset = String(RING_CIRCUMFERENCE);
    this.phase = "idle";
    this.el.dataset.state = "idle";
  }

  private onPointerDown = (e: PointerEvent): void => {
    if (e.button !== 0) return;
    if (this.phase !== "idle") this.resetVisual();
    window.clearTimeout(this.feedbackTimer);
    window.clearTimeout(this.dragWatchdog);
    primeAudio();

    this.phase = "pending";
    this.locked = false;
    this.reached = false;
    this.downPerf = performance.now();
    this.downWall = Date.now();
    this.downX = e.clientX;
    this.downY = e.clientY;

    this.el.setPointerCapture(e.pointerId);
    this.el.dataset.state = "holding";
    this.el.classList.remove("shake", "success", "reached");
    this.el.classList.add("rumble");
    this.readout.hidden = false;
    this.readout.textContent = "0.0s";
    this.hint.hidden = true;

    this.setInRep(true);
    this.beginFrame();
  };

  private onPointerMove = (e: PointerEvent): void => {
    if (this.phase !== "pending" || this.locked) return;
    const dx = e.clientX - this.downX;
    const dy = e.clientY - this.downY;
    if (Math.hypot(dx, dy) <= DRAG_PX) return;

    // 1 秒内移动超过阈值 -> 视为拖动窗口，取消本次计时
    this.phase = "dragging";
    this.stopFrame();
    this.el.releasePointerCapture?.(e.pointerId);
    this.setInRep(false);
    this.resetVisual();
    this.phase = "dragging";
    this.opts.onDragStart();
    this.dragWatchdog = window.setTimeout(() => {
      if (this.phase === "dragging") this.resetVisual();
    }, 1800);
  };

  private onPointerUp = (e: PointerEvent): void => {
    if (this.phase === "dragging") {
      this.resetVisual();
      return;
    }
    if (this.phase !== "pending") return;

    const elapsed = performance.now() - this.downPerf;
    const moved = Math.hypot(e.clientX - this.downX, e.clientY - this.downY);
    this.stopFrame();
    this.setInRep(false);

    if (elapsed < TAP_MS && moved <= DRAG_PX) {
      this.resetVisual();
      this.opts.onTap();
      return;
    }

    if (elapsed <= MIN_VALID_MS) {
      this.invalid(elapsed);
      return;
    }

    this.succeed(elapsed);
  };

  private onPointerCancel = (): void => {
    if (this.phase === "pending" && this.locked) {
      this.succeed(performance.now() - this.downPerf);
      return;
    }
    this.setInRep(false);
    this.resetVisual();
  };

  private onLostCapture = (): void => {
    if (this.phase === "pending" && this.locked) {
      this.succeed(performance.now() - this.downPerf);
      return;
    }
    if (this.phase === "pending") {
      this.setInRep(false);
      this.resetVisual();
    }
  };

  private onWindowBlur = (): void => {
    if (this.phase === "pending" && this.locked) {
      this.succeed(performance.now() - this.downPerf);
    } else if (this.phase === "pending") {
      this.setInRep(false);
      this.resetVisual();
    }
  };

  private beginFrame(): void {
    this.stopFrame();
    const frame = (): void => {
      if (this.phase !== "pending") return;
      const elapsed = performance.now() - this.downPerf;
      const progress = Math.min(1, elapsed / this.targetMs);
      this.ringFill.style.strokeDashoffset = String(
        RING_CIRCUMFERENCE * (1 - progress)
      );
      this.readout.textContent = `${(elapsed / 1000).toFixed(1)}s`;

      if (!this.locked && elapsed >= MIN_VALID_MS) {
        this.locked = true;
      }

      if (!this.reached && elapsed >= this.targetMs) {
        this.reached = true;
        this.el.classList.add("reached", "strong");
        if (this.opts.getSoundEnabled()) playTone("success");
      }

      this.raf = requestAnimationFrame(frame);
    };
    this.raf = requestAnimationFrame(frame);
  }

  private stopFrame(): void {
    if (this.raf) {
      cancelAnimationFrame(this.raf);
      this.raf = 0;
    }
  }

  private succeed(elapsedMs: number): void {
    const duration = Math.round(elapsedMs);
    this.stopFrame();
    this.setInRep(false);
    this.phase = "feedback";
    this.el.classList.remove("rumble", "strong");
    this.el.classList.add("success", "reached");
    this.ringFill.style.strokeDashoffset = "0";
    this.readout.textContent = `✓ ${formatSeconds(duration)}`;
    this.opts.onRecord(this.downWall, duration);

    this.feedbackTimer = window.setTimeout(() => this.resetVisual(), 950);
  }

  private invalid(elapsedMs: number): void {
    this.stopFrame();
    this.setInRep(false);
    this.phase = "feedback";
    this.el.classList.remove("rumble", "strong");
    void this.el.offsetWidth;
    this.el.classList.add("shake");
    const ms = Math.round(elapsedMs);
    this.readout.textContent =
      ms > TAP_MS ? `✗ ${(ms / 1000).toFixed(1)}s 太短` : "✗ 太短了";
    this.ringFill.style.strokeDashoffset = String(RING_CIRCUMFERENCE);

    this.feedbackTimer = window.setTimeout(() => this.resetVisual(), 900);
  }

  private setInRep(value: boolean): void {
    api.setInRep(value).catch((err) => this.opts.onError(err));
  }
}
