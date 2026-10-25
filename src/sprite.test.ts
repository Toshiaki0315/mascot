// npm test で実行する（Node の型の取り除きで動かすので DOM は使わない）
import assert from "node:assert/strict";
import { test } from "node:test";
import { hourLine, lineFor } from "./speech.ts";
import { readFileSync } from "node:fs";
import { CHARACTERS, usedColors, validateSprites } from "./sprite.ts";

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

test("ドット絵が使うすべての色に、CSS の塗りの指定がある", () => {
  const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
  for (const color of usedColors()) {
    assert.ok(css.includes(`.px-${color} `), `.px-${color} がありません`);
  }
});

test("じぶんの絵の語尾は「よ」", () => {
  assert.equal(hourLine(15, "custom"), "15 時だよ");
});
