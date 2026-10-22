import assert from "node:assert/strict";
import { test } from "node:test";
import {
  formatLead,
  formatWhen,
  sortReminders,
  toChoice,
  toSaved,
  WEEKDAYS,
  type Reminder,
} from "./reminder-format.ts";

// 2026-10-19 は月曜日（ローカル時刻で組み立てるので、実行する地域に左右されない）
const MON_9 = new Date(2026, 9, 19, 9, 5).getTime();

const r = (patch: Partial<Reminder>): Reminder => ({
  id: 1,
  text: "会議",
  at: MON_9,
  repeat: "once",
  days: 0,
  lead_min: 0,
  timer: false,
  ...patch,
});

test("1 回きりは日時だけ", () => {
  assert.equal(formatWhen(r({})), "10/19 09:05");
});

test("くり返しは種類と次の日付を出す", () => {
  assert.equal(formatWhen(r({ repeat: "daily" })), "毎日 09:05（次は 10/19）");
  assert.equal(formatWhen(r({ repeat: "weekly" })), "毎週月曜 09:05（次は 10/19）");
  assert.equal(formatWhen(r({ repeat: "biweekly" })), "隔週月曜 09:05（次は 10/19）");
  assert.equal(formatWhen(r({ repeat: "monthly", days: 19 })), "毎月19日 09:05（次は 10/19）");
});

test("曜日指定は曜日の名前、月〜金なら「平日」", () => {
  assert.equal(formatWhen(r({ repeat: "days", days: WEEKDAYS })), "平日 09:05（次は 10/19）");
  assert.equal(
    formatWhen(r({ repeat: "days", days: 0b101_0101 })),
    "月水金日 09:05（次は 10/19）",
  );
});

test("日曜日の名前は日（getDay が 0 でも）", () => {
  const sun = new Date(2026, 9, 25, 8, 0).getTime();
  assert.equal(formatWhen(r({ at: sun, repeat: "weekly" })), "毎週日曜 08:00（次は 10/25）");
});

test("画面の選択肢と保存する値は行き来できる", () => {
  assert.deepEqual(toSaved("weekdays", 0), { repeat: "days", days: WEEKDAYS });
  assert.deepEqual(toSaved("days", 0b110), { repeat: "days", days: 0b110 });
  assert.deepEqual(toSaved("daily", 0b111), { repeat: "daily", days: 0 });
  assert.deepEqual(toSaved("monthly", 0), { repeat: "monthly", days: 0 });
  assert.equal(toChoice({ repeat: "days", days: WEEKDAYS }), "weekdays");
  assert.equal(toChoice({ repeat: "days", days: 0b110 }), "days");
  assert.equal(toChoice({ repeat: "biweekly", days: 0 }), "biweekly");
});

test("並べ替えは日時順と登録順（元の配列は変えない）", () => {
  const list = [r({ id: 1, at: 300 }), r({ id: 2, at: 100 }), r({ id: 3, at: 200 })];
  assert.deepEqual(
    sortReminders(list, "time").map((x) => x.id),
    [2, 3, 1],
  );
  assert.deepEqual(
    sortReminders(list, "added").map((x) => x.id),
    [1, 2, 3],
  );
  assert.deepEqual(
    list.map((x) => x.id),
    [1, 2, 3],
  );
});

test("同じ日時なら登録順", () => {
  const list = [r({ id: 2, at: 100 }), r({ id: 1, at: 100 })];
  assert.deepEqual(
    sortReminders(list, "time").map((x) => x.id),
    [1, 2],
  );
});

test("事前に知らせる時間の表示", () => {
  assert.equal(formatLead(0), "");
  assert.equal(formatLead(10), "10 分前にも通知");
  assert.equal(formatLead(60), "1 時間前にも通知");
  assert.equal(formatLead(120), "2 時間前にも通知");
});
