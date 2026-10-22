import assert from "node:assert/strict";
import { test } from "node:test";
import { formatCountdown, phaseLabel } from "./timer-format.ts";

test("残り時間は mm:ss（1 時間以上は h:mm:ss）", () => {
  assert.equal(formatCountdown(0), "0:00");
  assert.equal(formatCountdown(59_000), "0:59");
  assert.equal(formatCountdown(61_000), "1:01");
  assert.equal(formatCountdown(25 * 60_000), "25:00");
  assert.equal(formatCountdown(3_600_000), "1:00:00");
  assert.equal(formatCountdown(3_723_000), "1:02:03");
});

test("端数の秒は切り上げ、過ぎていれば 0:00", () => {
  assert.equal(formatCountdown(1), "0:01");
  assert.equal(formatCountdown(-5000), "0:00");
});

test("区間の名前", () => {
  assert.equal(phaseLabel("work"), "作業中");
  assert.equal(phaseLabel("break"), "休憩中");
  assert.equal(phaseLabel("longbreak"), "長い休憩中");
});
