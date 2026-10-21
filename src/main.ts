import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Bubble, hourLine, lineFor, truncateForBubble, type Occasion } from "./speech";
import { CHARACTERS, renderSprite, type Character } from "./sprite";

type CursorPayload = { x: number; y: number };
type MovePayload = { moving: boolean; dir: -1 | 0 | 1 };
type FlagPayload = { on: boolean };
type SettingsPayload = { chatty: boolean; character: string };
type SayPayload = { kind: "break" };

/// 時刻になったリマインダー（表示待ちの列）
const pendingReminders: string[] = [];
/// いまふきだしに出しているリマインダーの全文（「あとで」を押したときに使う）
let currentReminder = "";

const stage = document.querySelector<HTMLElement>("#stage")!;
const mascot = document.querySelector<SVGSVGElement>("#mascot")!;
const bubbleEl = document.querySelector<HTMLElement>("#bubble")!;
const bubble = new Bubble(bubbleEl);

let character: Character = "tora";
let chatty = true;
renderSprite(mascot, character);

let overMascot = false;
let walking = false;
let flying = false;
let sleeping = false;
let suppressed = false;

function say(occasion: Occasion) {
  if (!chatty || suppressed) return;
  const line = lineFor(occasion, character);
  if (line) bubble.say(line);
}

// リマインダーを 1 件ずつ、クリックされたら次を出す。
// 全画面アプリの上で隠れている間は出さず、戻ってから出す
function showNextReminder() {
  if (suppressed || bubble.isPinned) return;
  const text = pendingReminders.shift();
  if (text === undefined) return;
  currentReminder = text;
  rejoice();
  bubble.remind(truncateForBubble(text));
}

// ふきだしを閉じて、次のリマインダーがあれば出す。snooze なら 5 分後にもう一度知らせる
function closeReminder(snooze: boolean) {
  if (!bubble.isPinned) return;
  if (snooze) void invoke("snooze_reminder", { text: currentReminder });
  bubble.dismiss();
  showNextReminder();
}

// 指定座標（ウィンドウ内の CSS ピクセル）がキャラクターの不透明部分か
function hitTest(x: number, y: number): boolean {
  if (suppressed) return false;
  if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) {
    return false;
  }
  const el = document.elementFromPoint(x, y);
  return isMascotHit(el) || isReminderBubble(el);
}

function isMascotHit(el: Element | null): boolean {
  return el instanceof SVGElement && el.classList.contains("hit");
}

// リマインダーのふきだしの上か（クリックして閉じられる）
function isReminderBubble(el: Element | null): boolean {
  return el instanceof HTMLElement && el.closest("#bubble.reminder") !== null;
}

async function updateHover(x: number, y: number) {
  const hit = hitTest(x, y);
  if (hit === overMascot) return;
  overMascot = hit;
  // ふきだしの上では、キャラクターは反応させない
  const overBody = hit && isMascotHit(document.elementFromPoint(x, y));
  mascot.classList.toggle("hover", overBody);
  if (hit) cancelEmote();
  // 不透明部分に乗ったらクリックを受け付け、離れたら再び透過する
  await invoke("set_click_through", { ignore: !hit });
}

// ---- 視線 ----

/// キャラクターの大きさの何倍の距離までカーソルを目で追うか
const LOOK_RANGE = 2.5;

function setLook(dir: -1 | 0 | 1) {
  mascot.classList.toggle("look-left", dir < 0);
  mascot.classList.toggle("look-right", dir > 0);
}

function lookAtCursor(x: number, y: number) {
  const rect = mascot.getBoundingClientRect();
  const dx = x - (rect.left + rect.width / 2);
  const dy = y - (rect.top + rect.height / 2);
  const near = Math.hypot(dx, dy) < rect.width * LOOK_RANGE;
  // 真上・真下あたりにいる時は正面を向く
  setLook(near && Math.abs(dx) > rect.width * 0.2 ? (dx < 0 ? -1 : 1) : 0);
}

// ---- 休憩中のしぐさ（あくび・伸び） ----

const EMOTES = ["yawn", "stretch"] as const;
const EMOTE_CHANCE = 0.5;
const EMOTE_DURATION_MS = 1400;
let emoteTimer: number | undefined;

function cancelEmote() {
  clearTimeout(emoteTimer);
  mascot.classList.remove(...EMOTES);
}

function isBusy() {
  return (
    walking ||
    flying ||
    sleeping ||
    suppressed ||
    overMascot ||
    mascot.classList.contains("happy") ||
    mascot.classList.contains("spinning")
  );
}

// 立ち止まったら、ときどき少し間を置いてしぐさをしたり、ひとこと話したりする
function onRest() {
  cancelEmote();
  say("rest");
  if (Math.random() >= EMOTE_CHANCE) return;
  emoteTimer = window.setTimeout(
    () => {
      if (isBusy()) return;
      const emote = EMOTES[Math.floor(Math.random() * EMOTES.length)];
      mascot.classList.add(emote);
      emoteTimer = window.setTimeout(() => mascot.classList.remove(emote), EMOTE_DURATION_MS);
    },
    1000 + Math.random() * 2000,
  );
}

// ---- 時報 ----

let lastHour = new Date().getHours();
window.setInterval(() => {
  const now = new Date();
  if (now.getMinutes() !== 0 || now.getHours() === lastHour) return;
  lastHour = now.getHours();
  if (chatty && !suppressed && !sleeping) bubble.say(hourLine(lastHour, character));
}, 20_000);

// ---- Rust 側からの通知 ----

// クリックスルー中は WebView にマウスイベントが届かないため、
// Rust 側がポーリングしたカーソル座標を受け取って判定する
await listen<CursorPayload>("cursor-moved", (e) => {
  updateHover(e.payload.x, e.payload.y);
  if (!walking && !sleeping && !flying) lookAtCursor(e.payload.x, e.payload.y);
});

// ランダム移動に合わせて、歩くアニメーションと視線を切り替える
await listen<MovePayload>("mascot-move", (e) => {
  const wasWalking = walking;
  walking = e.payload.moving;
  mascot.classList.toggle("walking", walking);
  if (walking) {
    cancelEmote();
    setLook(e.payload.dir);
  } else {
    setLook(0);
    if (wasWalking) onRest();
  }
});

// 投げられた・落ちている・窓へ飛び乗っている
await listen<FlagPayload>("mascot-fly", (e) => {
  const wasFlying = flying;
  flying = e.payload.on;
  mascot.classList.toggle("flying", flying);
  if (flying) {
    cancelEmote();
    setLook(0);
  } else if (wasFlying) {
    say("landed");
  }
});

await listen<FlagPayload>("mascot-sleep", (e) => {
  const wasSleeping = sleeping;
  sleeping = e.payload.on;
  mascot.classList.toggle("sleeping", sleeping);
  cancelEmote();
  setLook(0);
  if (sleeping) bubble.hide();
  else if (wasSleeping) say("wake");
});

await listen<FlagPayload>("mascot-suppress", (e) => {
  suppressed = e.payload.on;
  stage.classList.toggle("suppressed", suppressed);
  if (suppressed) {
    // 姿が見えなくなるので、出しかけのリマインダーは通知センターへ回す
    for (const text of [bubble.isPinned ? currentReminder : null, ...pendingReminders.splice(0)]) {
      if (text) void invoke("notify_reminder", { text });
    }
    bubble.dismiss();
    // Rust 側でクリックスルーに戻しているので、こちらの状態も合わせる
    overMascot = false;
    mascot.classList.remove("hover");
    cancelEmote();
    bubble.hide();
  }
});

await listen<SettingsPayload>("mascot-settings", (e) => {
  chatty = e.payload.chatty;
  if (!chatty) bubble.hide();
  const next = CHARACTERS.find((c) => c === e.payload.character) ?? "tora";
  if (next !== character) {
    character = next;
    renderSprite(mascot, character);
  }
});

await listen<string>("mascot-reminder", (e) => {
  // 全画面アプリの上などで姿が見えない間は、通知センターに出す
  if (suppressed) {
    void invoke("notify_reminder", { text: e.payload });
    return;
  }
  pendingReminders.push(e.payload);
  showNextReminder();
});

await listen<SayPayload>("mascot-say", (e) => {
  say(e.payload.kind);
});

// 受け取りの準備ができたので、起動直後に送られて取りこぼした状態を送り直してもらう
await invoke("frontend_ready");

// ---- クリック・ダブルクリック・ドラッグ ----

// 押してそのまま動かしたらドラッグ、その場で離したらクリックとして扱う。
// ドラッグして勢いよく離すと、Rust 側がその速さで投げる
const DRAG_THRESHOLD = 4;
const HAPPY_DURATION_MS = 1000;
const DOUBLE_CLICK_WAIT_MS = 250;
const SPIN_DURATION_MS = 800;
let pressAt: { x: number; y: number } | null = null;
let happyTimer: number | undefined;
let clickTimer: number | undefined;
let spinTimer: number | undefined;

function rejoice() {
  // 連打されたらアニメーションを最初からやり直す
  cancelEmote();
  mascot.classList.remove("happy");
  void mascot.getBoundingClientRect();
  mascot.classList.add("happy");
  clearTimeout(happyTimer);
  happyTimer = window.setTimeout(() => mascot.classList.remove("happy"), HAPPY_DURATION_MS);
  say("click");
}

function spin() {
  cancelEmote();
  mascot.classList.remove("happy", "spinning");
  void mascot.getBoundingClientRect();
  mascot.classList.add("spinning");
  clearTimeout(spinTimer);
  spinTimer = window.setTimeout(() => mascot.classList.remove("spinning"), SPIN_DURATION_MS);
  say("spin");
}

// リマインダーのふきだしをクリックしたら消して、次のリマインダーがあれば出す
bubbleEl.addEventListener("click", (e) => {
  const action = (e.target as HTMLElement).closest<HTMLElement>("[data-action]")?.dataset.action;
  closeReminder(action === "snooze");
});

// 右クリックで、トレイと同じメニューをその場に出す
window.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  if (!overMascot) return;
  pressAt = null;
  void invoke("show_context_menu");
});

mascot.addEventListener("mousedown", (e) => {
  if (e.button === 0 && overMascot) {
    pressAt = { x: e.clientX, y: e.clientY };
  }
});

window.addEventListener("mousemove", (e) => {
  if (!pressAt) return;
  if (Math.hypot(e.clientX - pressAt.x, e.clientY - pressAt.y) >= DRAG_THRESHOLD) {
    pressAt = null;
    bubble.hide();
    // ドラッグで動かした先から歩き出せるよう、Rust 側にドラッグの始まりを知らせる。
    // macOS のドラッグはマウスのイベントの最中に始める必要があるので、返事は待たない
    void invoke("start_drag");
    getCurrentWindow().startDragging();
  }
});

// シングルクリックはダブルクリックでないと分かるまで少し待ってから喜ぶ
window.addEventListener("mouseup", (e) => {
  if (pressAt && e.button === 0) {
    clearTimeout(clickTimer);
    if (bubble.isPinned) {
      // キャラクターをクリックしても、リマインダーを閉じられる
      closeReminder(false);
    } else if (e.detail >= 2) {
      spin();
    } else {
      clickTimer = window.setTimeout(rejoice, DOUBLE_CLICK_WAIT_MS);
    }
  }
  pressAt = null;
});
