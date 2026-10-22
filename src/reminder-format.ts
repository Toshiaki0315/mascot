// リマインダー登録画面の、DOM に触れない部分（表示用の文字列、曜日のビット、並べ替え）

export type Repeat = "once" | "daily" | "weekly" | "biweekly" | "monthly" | "days";
/// 画面の選択肢。「平日」は曜日指定（月〜金）として保存する
export type RepeatChoice = Repeat | "weekdays";
export type Reminder = { id: number; text: string; at: number; repeat: Repeat; days: number };

/// 曜日のビット（月曜 = bit 0 … 日曜 = bit 6）。Rust 側の reminder.rs と同じ
export const WEEKDAYS = 0b001_1111;
export const DAY_NAMES = ["月", "火", "水", "木", "金", "土", "日"];

const pad = (n: number) => String(n).padStart(2, "0");

export function daysLabel(mask: number): string {
  if (mask === WEEKDAYS) return "平日";
  return DAY_NAMES.filter((_, i) => mask & (1 << i)).join("");
}

/// 曜日の名前（月曜 = 0 … 日曜 = 6）。Date#getDay は日曜が 0 なので合わせる
function dayName(date: Date): string {
  return DAY_NAMES[(date.getDay() + 6) % 7];
}

/// 一覧に出す「いつ」の文字列。くり返すものは「次は …」も付ける
export function formatWhen(r: Reminder): string {
  const d = new Date(r.at);
  const date = `${d.getMonth() + 1}/${d.getDate()}`;
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  switch (r.repeat) {
    case "once":
      return `${date} ${time}`;
    case "daily":
      return `毎日 ${time}（次は ${date}）`;
    case "weekly":
      return `毎週${dayName(d)}曜 ${time}（次は ${date}）`;
    case "biweekly":
      return `隔週${dayName(d)}曜 ${time}（次は ${date}）`;
    case "monthly":
      return `毎月${r.days}日 ${time}（次は ${date}）`;
    case "days":
      return `${daysLabel(r.days)} ${time}（次は ${date}）`;
  }
}

/// 画面の選択肢から、保存する くり返し と 曜日のビットを決める
export function toSaved(choice: RepeatChoice, checkedDays: number): { repeat: Repeat; days: number } {
  if (choice === "weekdays") return { repeat: "days", days: WEEKDAYS };
  if (choice === "days") return { repeat: "days", days: checkedDays };
  // 毎月の日付は、最初の日付から Rust 側が決める
  return { repeat: choice, days: 0 };
}

/// 保存されている値を、画面の選択肢に戻す（編集用）
export function toChoice(r: Pick<Reminder, "repeat" | "days">): RepeatChoice {
  return r.repeat === "days" && r.days === WEEKDAYS ? "weekdays" : r.repeat;
}

export type SortOrder = "time" | "added";

export function sortReminders(list: Reminder[], order: SortOrder): Reminder[] {
  return [...list].sort((a, b) => (order === "time" ? a.at - b.at || a.id - b.id : a.id - b.id));
}
