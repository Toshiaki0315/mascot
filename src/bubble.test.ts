import assert from "node:assert/strict";
import { mock, test } from "node:test";
import { parseHTML } from "linkedom";
import { Bubble } from "./speech.ts";

// Bubble は DOM と window.setTimeout を使うので、軽い DOM（linkedom）と偽のタイマーで動かす
const { document } = parseHTML('<div id="bubble"></div>');
Object.assign(globalThis, {
  document,
  window: { setTimeout: (...args: Parameters<typeof setTimeout>) => setTimeout(...args) },
});

function setup() {
  mock.timers.enable({ apis: ["setTimeout"] });
  const el = document.getElementById("bubble")!;
  el.className = "";
  el.textContent = "";
  return { el, bubble: new Bubble(el as unknown as HTMLElement) };
}

test("ふつうのおしゃべりは出して、時間が来たら消える", () => {
  const { el, bubble } = setup();
  bubble.say("こんにちは");
  assert.equal(el.textContent, "こんにちは");
  assert.ok(el.classList.contains("show"));
  mock.timers.tick(3500);
  assert.ok(!el.classList.contains("show"));
  mock.timers.reset();
});

test("リマインダーは hide() では消えず、おしゃべりにも上書きされない", () => {
  const { el, bubble } = setup();
  bubble.remind("会議");
  assert.ok(bubble.isPinned);
  bubble.hide();
  bubble.say("じゃま");
  assert.ok(el.classList.contains("show"));
  assert.ok(el.textContent.startsWith("会議"));
  mock.timers.reset();
});

test("リマインダーのふきだしには「5 分後」と「閉じる」のボタンがある", () => {
  const { el, bubble } = setup();
  bubble.remind("会議");
  const actions = [...el.querySelectorAll("[data-action]")].map((b) => b.getAttribute("data-action"));
  assert.deepEqual(actions, ["snooze", "close"]);
  mock.timers.reset();
});

test("dismiss() で、リマインダーも消えて、おしゃべりできる状態に戻る", () => {
  const { el, bubble } = setup();
  bubble.remind("会議");
  bubble.dismiss();
  assert.ok(!bubble.isPinned);
  assert.ok(!el.classList.contains("show") && !el.classList.contains("reminder"));
  bubble.say("おはよう");
  assert.equal(el.textContent, "おはよう");
  mock.timers.reset();
});

test("リマインダーは短い文ほど大きな字（クラス）で出す", () => {
  const sizeOf = (text: string) => {
    const { el, bubble } = setup();
    bubble.remind(text);
    const cls = ["r-lg", "r-md", "r-sm"].find((c) => el.classList.contains(c));
    bubble.dismiss();
    mock.timers.reset();
    return cls;
  };
  assert.equal(sizeOf("会議"), "r-lg");
  assert.equal(sizeOf("あ".repeat(8)), "r-lg");
  assert.equal(sizeOf("あ".repeat(9)), "r-md");
  assert.equal(sizeOf("あ".repeat(16)), "r-md");
  assert.equal(sizeOf("あ".repeat(17)), "r-sm");
});

test("放っておいたリマインダーは 5 分で消える", () => {
  const { bubble } = setup();
  bubble.remind("会議");
  mock.timers.tick(5 * 60_000 - 1);
  assert.ok(bubble.isPinned);
  mock.timers.tick(1);
  assert.ok(!bubble.isPinned);
  mock.timers.reset();
});
