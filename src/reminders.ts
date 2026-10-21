import { invoke } from "@tauri-apps/api/core";

type Repeat = "once" | "daily" | "weekly";
type Reminder = { id: number; text: string; at: number; repeat: Repeat };

const REPEAT_LABEL: Record<Repeat, string> = { once: "", daily: "毎日", weekly: "毎週" };

const form = document.querySelector<HTMLFormElement>("#form")!;
const textInput = document.querySelector<HTMLInputElement>("#text")!;
const dateInput = document.querySelector<HTMLInputElement>("#date")!;
const timeInput = document.querySelector<HTMLInputElement>("#time")!;
const repeatSelect = document.querySelector<HTMLSelectElement>("#repeat")!;
const errorEl = document.querySelector<HTMLElement>("#error")!;
const listEl = document.querySelector<HTMLUListElement>("#list")!;
const emptyEl = document.querySelector<HTMLElement>("#empty")!;

const pad = (n: number) => String(n).padStart(2, "0");

/// 日付と時刻は別々の入力欄にする（datetime-local だと時刻を押してもカレンダーが開いてしまう）
function setWhen(date: Date) {
  dateInput.value = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  timeInput.value = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function formatWhen(at: number, repeat: Repeat): string {
  const d = new Date(at);
  const date = `${d.getMonth() + 1}/${d.getDate()}`;
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  return repeat === "once" ? `${date} ${time}` : `${REPEAT_LABEL[repeat]} ${time}（次は ${date}）`;
}

function render(list: Reminder[]) {
  listEl.replaceChildren(
    ...list.map((r) => {
      const li = document.createElement("li");
      const text = document.createElement("span");
      text.className = "text";
      text.textContent = r.text;
      const when = document.createElement("span");
      when.className = "when";
      when.textContent = formatWhen(r.at, r.repeat);
      const del = document.createElement("button");
      del.type = "button";
      del.textContent = "削除";
      del.addEventListener("click", async () => render(await invoke<Reminder[]>("delete_reminder", { id: r.id })));
      li.append(text, when, del);
      return li;
    }),
  );
  emptyEl.hidden = list.length > 0;
}

/// 日時の初期値は 1 時間後の 00 分
function resetForm() {
  textInput.value = "";
  repeatSelect.value = "once";
  const next = new Date(Date.now() + 60 * 60_000);
  next.setMinutes(0, 0, 0);
  setWhen(next);
}

for (const button of document.querySelectorAll<HTMLButtonElement>(".quick button")) {
  button.addEventListener("click", () => {
    setWhen(new Date(Date.now() + Number(button.dataset.min) * 60_000));
  });
}

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  errorEl.textContent = "";
  const at = new Date(`${dateInput.value}T${timeInput.value}`).getTime();
  if (Number.isNaN(at)) {
    errorEl.textContent = "日付と時刻を入力してください";
    return;
  }
  try {
    render(
      await invoke<Reminder[]>("add_reminder", {
        text: textInput.value,
        at,
        repeat: repeatSelect.value,
      }),
    );
    resetForm();
    textInput.focus();
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

resetForm();
render(await invoke<Reminder[]>("list_reminders"));
textInput.focus();
