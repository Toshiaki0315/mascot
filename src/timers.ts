import { invoke } from "@tauri-apps/api/core";
import type { Reminder } from "./reminder-format";
import { formatCountdown, phaseLabel, type Pomodoro, type PomodoroCfg } from "./timer-format";

const form = document.querySelector<HTMLFormElement>("#timer-form")!;
const labelInput = document.querySelector<HTMLInputElement>("#timer-label")!;
const minInput = document.querySelector<HTMLInputElement>("#timer-min")!;
const errorEl = document.querySelector<HTMLElement>("#timer-error")!;
const listEl = document.querySelector<HTMLUListElement>("#timer-list")!;
const emptyEl = document.querySelector<HTMLElement>("#timer-empty")!;

const statusEl = document.querySelector<HTMLElement>("#pomo-status")!;
const startButton = document.querySelector<HTMLButtonElement>("#pomo-start")!;
const stopButton = document.querySelector<HTMLButtonElement>("#pomo-stop")!;
const cfgForm = document.querySelector<HTMLFormElement>("#pomo-form")!;
const workInput = document.querySelector<HTMLInputElement>("#pomo-work")!;
const breakInput = document.querySelector<HTMLInputElement>("#pomo-break")!;
const longInput = document.querySelector<HTMLInputElement>("#pomo-long")!;
const everyInput = document.querySelector<HTMLInputElement>("#pomo-every")!;
const cfgError = document.querySelector<HTMLElement>("#pomo-error")!;

let timers: Reminder[] = [];
let pomodoro: Pomodoro | null = null;

function renderTimers() {
  const now = Date.now();
  listEl.replaceChildren(
    ...timers.map((t) => {
      const li = document.createElement("li");
      const text = document.createElement("span");
      text.className = "text";
      text.textContent = t.text;
      const left = document.createElement("span");
      left.className = "when";
      left.textContent = `残り ${formatCountdown(t.at - now)}`;
      const cancel = document.createElement("button");
      cancel.type = "button";
      cancel.textContent = "取り消す";
      cancel.addEventListener("click", async () => {
        await invoke("delete_reminder", { id: t.id });
        await refresh();
      });
      const wrap = document.createElement("span");
      wrap.className = "row-buttons";
      wrap.append(cancel);
      li.append(text, left, wrap);
      return li;
    }),
  );
  emptyEl.hidden = timers.length > 0;
}

function renderPomodoro() {
  if (pomodoro) {
    statusEl.textContent = `${phaseLabel(pomodoro.phase)}  残り ${formatCountdown(pomodoro.ends_at - Date.now())}（終わった作業 ${pomodoro.cycle} 回）`;
  } else {
    statusEl.textContent = "止まっています";
  }
  startButton.textContent = pomodoro ? "最初からやり直す" : "始める";
  stopButton.disabled = !pomodoro;
}

async function refresh() {
  const all = await invoke<Reminder[]>("list_reminders");
  timers = all.filter((r) => r.timer).sort((a, b) => a.at - b.at);
  const [p, cfg] = await invoke<[Pomodoro | null, PomodoroCfg]>("pomodoro_status");
  pomodoro = p;
  // 入力中の設定を上書きしないよう、欄にフォーカスがないときだけ入れ直す
  if (!cfgForm.contains(document.activeElement)) fillConfig(cfg);
  renderTimers();
  renderPomodoro();
}

function fillConfig(cfg: PomodoroCfg) {
  workInput.value = String(cfg.work_min);
  breakInput.value = String(cfg.break_min);
  longInput.value = String(cfg.long_break_min);
  everyInput.value = String(cfg.long_every);
}

for (const button of document.querySelectorAll<HTMLButtonElement>("[data-timer-min]")) {
  button.addEventListener("click", () => {
    minInput.value = button.dataset.timerMin ?? "3";
  });
}

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  errorEl.textContent = "";
  try {
    await invoke("start_timer", { minutes: Number(minInput.value), label: labelInput.value });
    labelInput.value = "";
    await refresh();
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

startButton.addEventListener("click", async () => {
  await invoke("pomodoro_start");
  await refresh();
});
stopButton.addEventListener("click", async () => {
  await invoke("pomodoro_stop");
  await refresh();
});

cfgForm.addEventListener("submit", async (e) => {
  e.preventDefault();
  cfgError.textContent = "";
  try {
    await invoke("set_pomodoro_config", {
      cfg: {
        work_min: Number(workInput.value),
        break_min: Number(breakInput.value),
        long_break_min: Number(longInput.value),
        long_every: Number(everyInput.value),
      },
    });
    await refresh();
  } catch (err) {
    cfgError.textContent = String(err);
  }
});

// 残り時間は 1 秒ごとに数え直し、状態（終わったか）は 5 秒ごとに問い合わせる
window.setInterval(() => {
  renderTimers();
  renderPomodoro();
}, 1000);
window.setInterval(() => void refresh(), 5000);
document.addEventListener("tab-shown", () => void refresh());
await refresh();
