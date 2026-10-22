import assert from "node:assert/strict";
import { mock, test } from "node:test";
import { Bubble, reminderActions } from "./speech.ts";

// Bubble は window.setTimeout を使うので、偽のタイマーで動かす
Object.assign(globalThis, {
  window: { setTimeout: (...args: Parameters<typeof setTimeout>) => setTimeout(...args) },
});

function setup() {
  mock.timers.enable({ apis: ["setTimeout"] });
  const shown: string[] = [];
  let hidden = 0;
  const bubble = new Bubble(
    (text) => shown.push(text),
    () => hidden++,
  );
  return { bubble, shown, hidden: () => hidden };
}

test("ふつうのおしゃべりは出して、時間が来たら消える", () => {
  const { bubble, shown, hidden } = setup();
  bubble.say("こんにちは");
  assert.deepEqual(shown, ["こんにちは"]);
  assert.equal(hidden(), 0);
  mock.timers.tick(3499);
  assert.equal(hidden(), 0);
  mock.timers.tick(1);
  assert.equal(hidden(), 1);
  mock.timers.reset();
});

test("続けて話すと、消えるまでの時間を数え直す", () => {
  const { bubble, shown, hidden } = setup();
  bubble.say("1");
  mock.timers.tick(3000);
  bubble.say("2");
  mock.timers.tick(3000);
  assert.equal(hidden(), 0);
  mock.timers.tick(500);
  assert.equal(hidden(), 1);
  assert.deepEqual(shown, ["1", "2"]);
  mock.timers.reset();
});

test("何も出していないときの hide() は、何もしない", () => {
  const { bubble, hidden } = setup();
  bubble.hide();
  assert.equal(hidden(), 0);
  mock.timers.reset();
});

test("出している間の hide() は 1 回だけ消し、時間が来ても二重に消さない", () => {
  const { bubble, hidden } = setup();
  bubble.say("こんにちは");
  bubble.hide();
  bubble.hide();
  mock.timers.tick(10_000);
  assert.equal(hidden(), 1);
  mock.timers.reset();
});

test("大きなふきだしのボタン：snooze のときだけ「5 分後」が付く", () => {
  assert.deepEqual(
    reminderActions(true).map(([action]) => action),
    ["snooze", "close"],
  );
  assert.deepEqual(
    reminderActions(false).map(([action]) => action),
    ["close"],
  );
});
