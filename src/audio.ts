let ctx: AudioContext | null = null;

/** 必须在用户手势中调用，避免自动播放策略拦截。 */
export function primeAudio(): void {
  if (!ctx) {
    try {
      ctx = new AudioContext();
    } catch {
      return;
    }
  }
  if (ctx.state === "suspended") {
    void ctx.resume().catch(() => undefined);
  }
}

type Tone = "tick" | "success" | "remind";

const TONES: Record<Tone, { freq: number; dur: number; gain: number; delay?: number }[]> = {
  tick: [{ freq: 660, dur: 0.05, gain: 0.05 }],
  success: [
    { freq: 880, dur: 0.09, gain: 0.08 },
    { freq: 1320, dur: 0.12, gain: 0.07, delay: 0.09 },
  ],
  remind: [
    { freq: 740, dur: 0.16, gain: 0.085 },
    { freq: 988, dur: 0.2, gain: 0.085, delay: 0.2 },
    { freq: 740, dur: 0.16, gain: 0.06, delay: 0.42 },
  ],
};

export function playTone(tone: Tone): void {
  if (!ctx || ctx.state !== "running") return;
  const now = ctx.currentTime;
  for (const spec of TONES[tone]) {
    const start = now + (spec.delay ?? 0);
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(spec.freq, start);
    gain.gain.setValueAtTime(0.0001, start);
    gain.gain.exponentialRampToValueAtTime(spec.gain, start + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + spec.dur);
    osc.connect(gain).connect(ctx.destination);
    osc.start(start);
    osc.stop(start + spec.dur + 0.02);
  }
}
