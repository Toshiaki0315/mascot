import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { renderSprite } from "./sprite";

type CursorPayload = { x: number; y: number };
type MovePayload = { moving: boolean; dir: -1 | 0 | 1 };
type FlagPayload = { on: boolean };

const mascot = document.querySelector<SVGSVGElement>("#mascot")!;
renderSprite(mascot);

let overMascot = false;
let walking = false;
let sleeping = false;
let suppressed = false;

// 指定座標（ウィンドウ内の CSS ピクセル）がキャラクターの不透明部分か
function hitTest(x: number, y: number): boolean {
  if (suppressed) return false;
  if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) {
    return false;
  }
  const el = document.elementFromPoint(x, y);
  return el instanceof SVGElement && el.classList.contains("hit");
}

async function updateHover(x: number, y: number) {
  const hit = hitTest(x, y);
  if (hit === overMascot) return;
  overMascot = hit;
  mascot.classList.toggle("hover", hit);
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

function canEmote() {
  return (
    !walking &&
    !sleeping &&
    !suppressed &&
    !overMascot &&
    !mascot.classList.contains("happy") &&
    !mascot.classList.contains("spinning")
  );
}

// 立ち止まったら、ときどき少し間を置いてしぐさをする
function maybeEmote() {
  cancelEmote();
  if (Math.random() >= EMOTE_CHANCE) return;
  emoteTimer = window.setTimeout(
    () => {
      if (!canEmote()) return;
      const emote = EMOTES[Math.floor(Math.random() * EMOTES.length)];
      mascot.classList.add(emote);
      emoteTimer = window.setTimeout(() => mascot.classList.remove(emote), EMOTE_DURATION_MS);
    },
    1000 + Math.random() * 2000,
  );
}

// ---- Rust 側からの通知 ----

// クリックスルー中は WebView にマウスイベントが届かないため、
// Rust 側がポーリングしたカーソル座標を受け取って判定する
await listen<CursorPayload>("cursor-moved", (e) => {
  updateHover(e.payload.x, e.payload.y);
  if (!walking && !sleeping) lookAtCursor(e.payload.x, e.payload.y);
});

// ランダム移動に合わせて、歩くアニメーションと視線を切り替える
await listen<MovePayload>("mascot-move", (e) => {
  walking = e.payload.moving;
  mascot.classList.toggle("walking", walking);
  if (walking) {
    cancelEmote();
    setLook(e.payload.dir);
  } else {
    setLook(0);
    maybeEmote();
  }
});

await listen<FlagPayload>("mascot-sleep", (e) => {
  sleeping = e.payload.on;
  mascot.classList.toggle("sleeping", sleeping);
  cancelEmote();
  setLook(0);
});

await listen<FlagPayload>("mascot-suppress", (e) => {
  suppressed = e.payload.on;
  mascot.classList.toggle("suppressed", suppressed);
  if (suppressed) {
    // Rust 側でクリックスルーに戻しているので、こちらの状態も合わせる
    overMascot = false;
    mascot.classList.remove("hover");
    cancelEmote();
  }
});

// 受け取りの準備ができたので、起動直後に送られて取りこぼした状態を送り直してもらう
await invoke("frontend_ready");

// ---- クリック・ダブルクリック・ドラッグ ----

// 押してそのまま動かしたらドラッグ、その場で離したらクリックとして扱う
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
}

function spin() {
  cancelEmote();
  mascot.classList.remove("happy", "spinning");
  void mascot.getBoundingClientRect();
  mascot.classList.add("spinning");
  clearTimeout(spinTimer);
  spinTimer = window.setTimeout(() => mascot.classList.remove("spinning"), SPIN_DURATION_MS);
}

mascot.addEventListener("mousedown", (e) => {
  if (e.button === 0 && overMascot) {
    pressAt = { x: e.clientX, y: e.clientY };
  }
});

window.addEventListener("mousemove", (e) => {
  if (!pressAt) return;
  if (Math.hypot(e.clientX - pressAt.x, e.clientY - pressAt.y) >= DRAG_THRESHOLD) {
    pressAt = null;
    getCurrentWindow().startDragging();
  }
});

// シングルクリックはダブルクリックでないと分かるまで少し待ってから喜ぶ
window.addEventListener("mouseup", (e) => {
  if (pressAt && e.button === 0) {
    clearTimeout(clickTimer);
    if (e.detail >= 2) {
      spin();
    } else {
      clickTimer = window.setTimeout(rejoice, DOUBLE_CLICK_WAIT_MS);
    }
  }
  pressAt = null;
});
