// ふきだしのセリフと表示
import type { Character } from "./sprite";

export type Occasion =
  | "click" // クリックされた
  | "spin" // ダブルクリックで回った
  | "rest" // 歩き終わって休んでいる
  | "landed" // 投げられて着地した
  | "wake" // 目を覚ました
  | "break"; // 休憩をすすめる

/// その場面で話す確率（break は必ず話す）
const CHANCE: Record<Occasion, number> = {
  click: 0.3,
  spin: 0.5,
  rest: 0.08,
  landed: 0.5,
  wake: 0.5,
  break: 1,
};

const LINES: Record<Occasion, string[]> = {
  click: ["えへへ", "うれしい！", "もっとなでて〜"],
  spin: ["くるりん！", "めがまわる〜"],
  rest: ["ふぅ", "なにしてるの？", "いい天気だね", "おなかすいたな"],
  landed: ["びっくりした〜", "もう一回！", "とうちゃく！"],
  wake: ["おはよう！", "ねてないよ…"],
  break: ["そろそろ休憩しよう？", "ちょっと伸びをしようよ"],
};

/// 鳴き声と語尾。いぬは「わん」、ネコは「にゃ」
function voice(character: Character) {
  return character === "inu" ? { cry: "わん！", tail: "わん" } : { cry: "にゃーん", tail: "にゃ" };
}

function pick<T>(items: T[]): T {
  return items[Math.floor(Math.random() * items.length)];
}

export function lineFor(occasion: Occasion, character: Character): string | null {
  if (Math.random() >= CHANCE[occasion]) return null;
  const { cry } = voice(character);
  // 休憩中は、ときどき鳴き声だけ
  if (occasion === "rest" && Math.random() < 0.3) return cry;
  return pick(LINES[occasion]);
}

export function hourLine(hour: number, character: Character): string {
  const { tail } = voice(character);
  return `${hour} 時だ${tail}`;
}

const SHOW_MS = 3500;

/// 大きなふきだしのボタン（[動作, 表示]）。`snooze` が false なら、「5 分後」は付けない
/// （タイマーや天気など、あとで知らせ直す意味がないもの）
export function reminderActions(snooze: boolean): [string, string][] {
  return snooze
    ? [
        ["snooze", "5 分後"],
        ["close", "閉じる"],
      ]
    : [["close", "閉じる"]];
}

/// ふつうのおしゃべりのふきだし。表示と消去は、渡された関数（大きなふきだしのウィンドウを動かす）で行い、
/// ここでは「何秒たったら消すか」と、出ているかどうかを持つ
export class Bubble {
  private timer: number | undefined;
  private shown = false;
  private readonly show: (text: string) => void;
  private readonly hideNow: () => void;

  constructor(show: (text: string) => void, hideNow: () => void) {
    this.show = show;
    this.hideNow = hideNow;
  }

  say(text: string) {
    this.shown = true;
    this.show(text);
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.hide(), SHOW_MS);
  }

  hide() {
    clearTimeout(this.timer);
    if (!this.shown) return;
    this.shown = false;
    this.hideNow();
  }
}
