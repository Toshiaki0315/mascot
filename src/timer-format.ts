// タイマーとポモドーロの表示用（DOM に触れない部分）

export type Phase = "work" | "break" | "longbreak";
export type Pomodoro = { phase: Phase; ends_at: number; cycle: number };
export type PomodoroCfg = {
  work_min: number;
  break_min: number;
  long_break_min: number;
  long_every: number;
};

/// 残りミリ秒を「mm:ss」（1 時間以上なら「h:mm:ss」）にする。過ぎていれば 0:00
export function formatCountdown(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function phaseLabel(phase: Phase): string {
  return phase === "work" ? "作業中" : phase === "break" ? "休憩中" : "長い休憩中";
}
