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

/// ふきだしに出せるリマインダーの文字数。超えたぶんは「…」で省く
/// （全文は Rust 側が通知センターに出す。reminder.rs の BUBBLE_CHARS と同じ値）
export const BUBBLE_CHARS = 30;

export function truncateForBubble(text: string): string {
  const chars = [...text];
  return chars.length <= BUBBLE_CHARS ? text : `${chars.slice(0, BUBBLE_CHARS - 1).join("")}…`;
}

/// リマインダーのふきだしは、クリックされるまで（または長く放置されるまで）出し続ける
const REMIND_MS = 5 * 60_000;

export class Bubble {
  private readonly el: HTMLElement;
  private timer: number | undefined;
  private pinned = false;

  constructor(el: HTMLElement) {
    this.el = el;
  }

  /// リマインダーを出している間か
  get isPinned() {
    return this.pinned;
  }

  say(text: string) {
    // リマインダーを出している間は、ふつうのおしゃべりで上書きしない
    if (this.pinned) return;
    this.show(text, SHOW_MS);
  }

  /// リマインダーを出す。ふつうのおしゃべりや hide() では消えない。
  /// 文字の下に「5 分後」「閉じる」のボタンを付ける（押したかどうかは data-action で見分ける）。
  /// `snooze` が false なら、「5 分後」は付けない（タイマーや天気など、あとで知らせ直す意味がないもの）
  remind(text: string, snooze = true) {
    this.pinned = true;
    // 短い文ほど大きな字で出す
    const length = [...text].length;
    this.el.classList.remove("r-lg", "r-md", "r-sm");
    this.el.classList.add("reminder", length <= 8 ? "r-lg" : length <= 16 ? "r-md" : "r-sm");

    const message = document.createElement("div");
    message.textContent = text;
    const actions = document.createElement("div");
    actions.className = "actions";
    const buttons = snooze ? [["snooze", "5 分後"], ["close", "閉じる"]] : [["close", "閉じる"]];
    for (const [action, label] of buttons) {
      const button = document.createElement("span");
      button.className = "action";
      button.dataset.action = action;
      button.textContent = label;
      actions.append(button);
    }
    this.show([message, actions], REMIND_MS);
  }

  private show(content: string | HTMLElement[], ms: number) {
    if (typeof content === "string") this.el.textContent = content;
    else this.el.replaceChildren(...content);
    // 表示中に次のセリフが来たら、ポップのアニメーションからやり直す
    this.el.classList.remove("show");
    void this.el.offsetWidth;
    this.el.classList.add("show");
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.dismiss(), ms);
  }

  hide() {
    if (this.pinned) return;
    this.dismiss();
  }

  /// リマインダーも含めて消す
  dismiss() {
    clearTimeout(this.timer);
    this.pinned = false;
    this.el.classList.remove("show", "reminder", "r-lg", "r-md", "r-sm");
  }
}
