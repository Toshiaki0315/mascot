import { invoke } from "@tauri-apps/api/core";
import {
  formatLead,
  formatWhen,
  sortReminders,
  toChoice,
  toSaved,
  type Reminder,
  type RepeatChoice,
  type SortOrder,
} from "./reminder-format";

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
const sortSelect = document.querySelector<HTMLSelectElement>("#sort")!;
const leadSelect = document.querySelector<HTMLSelectElement>("#lead")!;

/// 編集中のリマインダーの ID（新規登録なら null）
let editingId: number | null = null;
let current: Reminder[] = [];

const pad = (n: number) => String(n).padStart(2, "0");

/// 日付と時刻は別々の入力欄にする（datetime-local だと時刻を押してもカレンダーが開いてしまう）
function setWhen(date: Date) {
  dateInput.value = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  timeInput.value = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
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

function render(all: Reminder[]) {
  // カウントダウンのタイマーは「タイマー」の見出しに出す
  const list = all.filter((r) => !r.timer);
  current = all;
  listEl.replaceChildren(
    ...sortReminders(list, sortSelect.value as SortOrder).map((r) => {
      const li = document.createElement("li");
      li.classList.toggle("editing", r.id === editingId);
      const text = document.createElement("span");
      text.className = "text";
      text.textContent = r.text;
      const when = document.createElement("span");
      when.className = "when";
      when.textContent = [formatWhen(r), formatLead(r.lead_min)].filter(Boolean).join("  ・  ");

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
  leadSelect.value = "0";
  syncDaysBox();
  const next = new Date(Date.now() + 60 * 60_000);
  next.setMinutes(0, 0, 0);
  setWhen(next);
}

function startEdit(r: Reminder) {
  editingId = r.id;
  textInput.value = r.text;
  setWhen(new Date(r.at));
  repeatSelect.value = toChoice(r);
  setDays(r.days);
  leadSelect.value = String(r.lead_min);
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
sortSelect.addEventListener("change", () => {
  try {
    localStorage.setItem("reminder-sort", sortSelect.value);
  } catch {
    // 保存できなくても、並べ方が次回に戻るだけ
  }
  render(current);
});
cancelButton.addEventListener("click", stopEdit);

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  errorEl.textContent = "";
  const at = new Date(`${dateInput.value}T${timeInput.value}`).getTime();
  if (Number.isNaN(at)) {
    errorEl.textContent = "日付と時刻を入力してください";
    return;
  }
  const { repeat, days } = toSaved(repeatSelect.value as RepeatChoice, selectedDays());
  try {
    const args = { text: textInput.value, at, repeat, days, leadMin: Number(leadSelect.value) };
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

try {
  const saved = localStorage.getItem("reminder-sort");
  if (saved === "time" || saved === "added") sortSelect.value = saved;
} catch {
  // 読めなければ日時順
}
resetForm();
render(await invoke<Reminder[]>("list_reminders"));
textInput.focus();
