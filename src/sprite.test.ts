// npm test で実行する（Node の型の取り除きで動かすので DOM は使わない）
import assert from "node:assert/strict";
import { test } from "node:test";
import { BUBBLE_CHARS, hourLine, lineFor, truncateForBubble } from "./speech.ts";
import { CHARACTERS, validateSprites } from "./sprite.ts";

test("すべてのドット絵の行が 16 ドット幅", () => {
  assert.deepEqual(validateSprites(), []);
});

test("休憩のすすめは必ず話す", () => {
  for (const c of CHARACTERS) {
    assert.ok(lineFor("break", c));
  }
});

test("時報の語尾はキャラクターで変わる", () => {
  assert.equal(hourLine(15, "tora"), "15 時だにゃ");
  assert.equal(hourLine(9, "inu"), "9 時だわん");
});

test("ふきだしに収まらない長いリマインダーは「…」で省く", () => {
  assert.equal(truncateForBubble("会議"), "会議");
  const exact = "あ".repeat(BUBBLE_CHARS);
  assert.equal(truncateForBubble(exact), exact);
  const cut = truncateForBubble("あ".repeat(BUBBLE_CHARS + 10));
  assert.equal([...cut].length, BUBBLE_CHARS);
  assert.ok(cut.endsWith("…"));
});
