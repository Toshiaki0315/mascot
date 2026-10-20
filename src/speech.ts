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

export class Bubble {
  private readonly el: HTMLElement;
  private timer: number | undefined;

  constructor(el: HTMLElement) {
    this.el = el;
  }

  say(text: string) {
    this.el.textContent = text;
    // 表示中に次のセリフが来たら、ポップのアニメーションからやり直す
    this.el.classList.remove("show");
    void this.el.offsetWidth;
    this.el.classList.add("show");
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.hide(), SHOW_MS);
  }

  hide() {
    clearTimeout(this.timer);
    this.el.classList.remove("show");
  }
}
