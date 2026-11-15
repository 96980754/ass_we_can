function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
}

interface Step {
  title: string;
  body: string;
  warn?: string;
}

/** 一次性新手教程：3 分钟内完成，重点解决「找不到肌肉 / 练错部位」。 */
const TUTORIAL: Step[] = [
  {
    title: "第 1 步 · 憋住放屁",
    body:
      "想象你正在忍住一个屁：肛门周围轻轻向上收紧，像把气往回提。保持 3 秒，然后完全放松。",
  },
  {
    title: "第 2 步 · 中断尿流",
    body:
      "想象排尿到一半时把它中断，感受那块向上抬起的肌肉。",
    warn:
      "只用来找感觉：不要真的在排尿时反复练习，长期这样做可能影响正常排尿。",
  },
  {
    title: "第 3 步 · 会阴感知",
    body:
      "洗净手后，用一根手指轻按会阴（肛门与阴囊 / 阴道之间）。收缩时，你应该能感到那里轻轻向上抬起。",
  },
  {
    title: "第 4 步 · 确认位置",
    body: "正确的收缩是「会阴向上提」，而不是夹紧臀部。",
    warn:
      "如果臀部夹紧、肚子鼓起或大腿发力，说明练错部位了。先完全放松，再试一次。",
  },
];

type Mode = "none" | "breath" | "tutorial";

export class Onboarding {
  private readonly el = byId("modal");
  private readonly kicker = byId("modal-kicker");
  private readonly title = byId("modal-title");
  private readonly body = byId("modal-body");
  private readonly warn = byId("modal-warn");
  private readonly actions = byId("modal-actions");
  private readonly progress = byId("modal-progress");

  private mode: Mode = "none";
  private step = 0;
  private finish: (() => void) | null = null;

  get isOpen(): boolean {
    return this.mode !== "none";
  }

  /** 首次强制提示：不要憋气。必须点「我知道了」才能开始训练。 */
  showBreathNotice(onAcknowledge: () => void): void {
    this.mode = "breath";
    this.finish = onAcknowledge;
    this.kicker.textContent = "开始之前 · 务必先看";
    this.title.textContent = "训练时，不要憋气";
    this.body.innerHTML =
      "<p>收缩时缓慢<strong>呼气</strong>，放松时<strong>吸气</strong>。圆环会跟着呼吸缩放。</p>" +
      "<p>憋气会把腹压往下推，让「提肛」变成「往下压」，越练越错。</p>";
    this.warn.hidden = true;
    this.progress.textContent = "";
    this.actions.replaceChildren();
    const ok = this.button("我知道了", "primary", () => this.complete());
    this.actions.append(ok);
    this.el.hidden = false;
  }

  /** 4 步肌肉定位教程（一次性，可跳过）。 */
  showTutorial(onDone: () => void, allowSkip = true): void {
    this.mode = "tutorial";
    this.step = 0;
    this.finish = onDone;
    this.el.hidden = false;
    this.renderStep(allowSkip);
  }

  hide(): void {
    this.mode = "none";
    this.finish = null;
    this.el.hidden = true;
  }

  /** 先取出回调再关闭，避免 hide() 清空回调导致后续流程丢失 */
  private complete(): void {
    const done = this.finish;
    this.hide();
    done?.();
  }

  private renderStep(allowSkip: boolean): void {
    const step = TUTORIAL[this.step];
    this.kicker.textContent = "新手教程 · 约 3 分钟";
    this.title.textContent = step.title;
    this.body.innerHTML = `<p>${step.body}</p>`;
    this.warn.hidden = !step.warn;
    this.warn.textContent = step.warn ?? "";
    this.progress.textContent = `第 ${this.step + 1} / ${TUTORIAL.length} 步`;

    this.actions.replaceChildren();
    if (this.step > 0) {
      this.actions.append(
        this.button("上一步", "", () => {
          this.step -= 1;
          this.renderStep(allowSkip);
        })
      );
    }
    const isLast = this.step === TUTORIAL.length - 1;
    this.actions.append(
      this.button(isLast ? "完成教程" : "下一步", "primary", () => {
        if (isLast) {
          this.complete();
          return;
        }
        this.step += 1;
        this.renderStep(allowSkip);
      })
    );
    if (allowSkip) {
      this.actions.append(this.button("跳过", "ghost", () => this.complete()));
    }
  }

  private button(label: string, variant: string, onClick: () => void): HTMLButtonElement {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `btn ${variant}`.trim();
    btn.textContent = label;

    // 部分 WebView（WebKitGTK）在弹窗上对合成 click 不稳，pointerup 更可靠；
    // 同时保留 click 以支持键盘 Enter/Space。
    let fired = false;
    const fire = (event: Event): void => {
      if (fired) return;
      fired = true;
      event.preventDefault();
      onClick();
    };
    btn.addEventListener("pointerup", fire);
    btn.addEventListener("click", fire);
    return btn;
  }
}
