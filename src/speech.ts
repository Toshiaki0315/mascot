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

/// 鳴き声と語尾。いぬは「わん」、ネコは「にゃ」（くらにゃんぽいは「にゃん」）、じぶんの絵は「よ」
function voice(character: Character) {
  if (character === "inu") return { cry: "わん！", tail: "わん" };
  if (character === "kuranyan") return { cry: "にゃーん", tail: "にゃん" };
  // 「じぶんの絵」は、何の生き物かわからないので、ふつうの言い方にする
  if (character === "custom") return { cry: "…！", tail: "よ" };
  return { cry: "にゃーん", tail: "にゃ" };
}

function pick<T>(items: T[]): T {
  return items[Math.floor(Math.random() * items.length)];
}

export function lineFor(occasion: Occasion, character: Character): string | null {
  if (Math.random() >= CHANCE[occasion]) return null;
  const { cry } = voice(character);
  // 休憩中は、ときどき鳴き声だけ（鳴き声には口ぐせを付けない）
  if (occasion === "rest" && Math.random() < 0.3) return cry;
  return withHabit(pick(LINES[occasion]), character);
}

/// くらにゃんぽいでも「にゃん」を付けないセリフ（笑い声やため息は、そのままのほうが自然）
const NO_HABIT = new Set(["えへへ", "ふぅ"]);

/// キャラクターの口ぐせを付ける。いまは、くらにゃんぽいの文の最後に「にゃん」を付けるだけ。
///   - 最後の「よ」「ね」「な」は取ってから付ける（「したよにゃん」のような重なりを避ける）
///   - 最後に「！」「？」「〜」「…」や（かっこ）があれば、その前に付ける
///   - 「終わりました」は「終わった」に言い換えてから付ける
/// マスコット自身の言葉にだけ使う（自分で登録したリマインダーの文やエラーの文には使わない）
export function withHabit(text: string, character: Character): string {
  if (character !== "kuranyan" || NO_HABIT.has(text)) return text;
  const [, body, tail] = text.match(/^([\s\S]*?)((?:（[^（）]*）)?[！？〜…!?]*)$/) ?? ["", text, ""];
  const core = body.replace(/終わりました$/, "終わった").replace(/[よねな]$/, "");
  return `${core}にゃん${tail}`;
}

export function hourLine(hour: number, character: Character): string {
  const { tail } = voice(character);
  return `${hour} 時だ${tail}`;
}

const SHOW_MS = 3500;

/// 大きなふきだしのボタン（[動作, 表示]）。閉じるボタンはなく、ふきだしのどこかをクリックすると閉じる。
/// 「5 分後」は `snooze` が true のとき（リマインダー）だけ付ける。タイマーや天気など、
/// あとで知らせ直す意味がないものには、ボタンを付けない
export function reminderActions(snooze: boolean): [string, string][] {
  return snooze ? [["snooze", "5 分後にもう一度"]] : [];
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
