import { invoke } from "@tauri-apps/api/core";

type Repeat = "once" | "daily" | "weekly" | "days";
// 画面の選択肢。「平日」は曜日指定（月〜金）として保存する
type RepeatChoice = Repeat | "weekdays";
type Reminder = { id: number; text: string; at: number; repeat: Repeat; days: number };

/// 曜日のビット（月曜 = bit 0 … 日曜 = bit 6）。Rust 側の reminder.rs と同じ
const WEEKDAYS = 0b001_1111;
const DAY_NAMES = ["月", "火", "水", "木", "金", "土", "日"];

const form = document.querySelector<HTMLFormElement>("#form")!;
const textInput = document.querySelector<HTMLInputElement>("#text")!;
const dateInput = document.querySelector<HTMLInputElement>("#date")!;
const timeInput = document.querySelector<HTMLInputElement>("#time")!;
const repeatSelect = document.querySelector<HTMLSelectElement>("#repeat")!;
const daysBox = document.querySelector<HTMLElement>("#days")!;
const dayChecks = [...daysBox.querySelectorAll<HTMLInputElement>("input")];
const errorEl = document.querySelector<HTMLElement>("#error")!;
const addButton = document.querySelector<HTMLButtonElement>("#add")!;
const cancelButton = document.querySelector<HTMLButtonElement>("#cancel")!;
const listEl = document.querySelector<HTMLUListElement>("#list")!;
const emptyEl = document.querySelector<HTMLElement>("#empty")!;

/// 編集中のリマインダーの ID（新規登録なら null）
let editingId: number | null = null;
let current: Reminder[] = [];

const pad = (n: number) => String(n).padStart(2, "0");

/// 日付と時刻は別々の入力欄にする（datetime-local だと時刻を押してもカレンダーが開いてしまう）
function setWhen(date: Date) {
  dateInput.value = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  timeInput.value = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function daysLabel(mask: number): string {
  if (mask === WEEKDAYS) return "平日";
  return DAY_NAMES.filter((_, i) => mask & (1 << i)).join("");
}

function formatWhen(r: Reminder): string {
  const d = new Date(r.at);
  const date = `${d.getMonth() + 1}/${d.getDate()}`;
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  switch (r.repeat) {
    case "once":
      return `${date} ${time}`;
    case "daily":
      return `毎日 ${time}（次は ${date}）`;
    case "weekly":
      return `毎週${DAY_NAMES[(d.getDay() + 6) % 7]}曜 ${time}（次は ${date}）`;
    case "days":
      return `${daysLabel(r.days)} ${time}（次は ${date}）`;
  }
}

function selectedDays(): number {
  return dayChecks.reduce((mask, box) => (box.checked ? mask | (1 << Number(box.value)) : mask), 0);
}

function setDays(mask: number) {
  for (const box of dayChecks) box.checked = (mask & (1 << Number(box.value))) !== 0;
}

function syncDaysBox() {
  daysBox.hidden = repeatSelect.value !== "days";
}

function render(list: Reminder[]) {
  current = list;
  listEl.replaceChildren(
    ...list.map((r) => {
      const li = document.createElement("li");
      li.classList.toggle("editing", r.id === editingId);
      const text = document.createElement("span");
      text.className = "text";
      text.textContent = r.text;
      const when = document.createElement("span");
      when.className = "when";
      when.textContent = formatWhen(r);

      const edit = document.createElement("button");
      edit.type = "button";
      edit.className = "edit";
      edit.textContent = "編集";
      edit.addEventListener("click", () => startEdit(r));
      const del = document.createElement("button");
      del.type = "button";
      del.textContent = "削除";
      del.addEventListener("click", async () => {
        if (r.id === editingId) stopEdit();
        render(await invoke<Reminder[]>("delete_reminder", { id: r.id }));
      });
      const buttons = document.createElement("span");
      buttons.className = "row-buttons";
      buttons.append(edit, del);

      li.append(text, when, buttons);
      return li;
    }),
  );
  emptyEl.hidden = list.length > 0;
}

/// 日時の初期値は 1 時間後の 00 分
function resetForm() {
  textInput.value = "";
  repeatSelect.value = "once";
  setDays(0);
  syncDaysBox();
  const next = new Date(Date.now() + 60 * 60_000);
  next.setMinutes(0, 0, 0);
  setWhen(next);
}

function startEdit(r: Reminder) {
  editingId = r.id;
  textInput.value = r.text;
  setWhen(new Date(r.at));
  repeatSelect.value = r.repeat === "days" && r.days === WEEKDAYS ? "weekdays" : r.repeat;
  setDays(r.days);
  syncDaysBox();
  errorEl.textContent = "";
  addButton.textContent = "更新";
  cancelButton.hidden = false;
  render(current);
  textInput.focus();
}

function stopEdit() {
  editingId = null;
  addButton.textContent = "追加";
  cancelButton.hidden = true;
  errorEl.textContent = "";
  resetForm();
  render(current);
}

for (const button of document.querySelectorAll<HTMLButtonElement>(".quick button")) {
  button.addEventListener("click", () => {
    setWhen(new Date(Date.now() + Number(button.dataset.min) * 60_000));
  });
}

repeatSelect.addEventListener("change", syncDaysBox);
cancelButton.addEventListener("click", stopEdit);

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  errorEl.textContent = "";
  const at = new Date(`${dateInput.value}T${timeInput.value}`).getTime();
  if (Number.isNaN(at)) {
    errorEl.textContent = "日付と時刻を入力してください";
    return;
  }
  const choice = repeatSelect.value as RepeatChoice;
  const repeat: Repeat = choice === "weekdays" ? "days" : choice;
  const days = choice === "weekdays" ? WEEKDAYS : choice === "days" ? selectedDays() : 0;
  try {
    const args = { text: textInput.value, at, repeat, days };
    const list =
      editingId === null
        ? await invoke<Reminder[]>("add_reminder", args)
        : await invoke<Reminder[]>("update_reminder", { id: editingId, ...args });
    editingId = null;
    addButton.textContent = "追加";
    cancelButton.hidden = true;
    render(list);
    resetForm();
    textInput.focus();
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

resetForm();
render(await invoke<Reminder[]>("list_reminders"));
textInput.focus();
