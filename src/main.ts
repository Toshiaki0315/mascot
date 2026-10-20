import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { renderSprite } from "./sprite";

type CursorPayload = { x: number; y: number };
type MovePayload = { moving: boolean; dir: -1 | 0 | 1 };

const mascot = document.querySelector<SVGSVGElement>("#mascot")!;
renderSprite(mascot);
let overMascot = false;

// 指定座標（ウィンドウ内の CSS ピクセル）がキャラクターの不透明部分か
function hitTest(x: number, y: number): boolean {
  if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) {
    return false;
  }
  const el = document.elementFromPoint(x, y);
  return el instanceof SVGElement && el.classList.contains("hit");
}

async function update(x: number, y: number) {
  const hit = hitTest(x, y);
  if (hit === overMascot) return;
  overMascot = hit;
  mascot.classList.toggle("hover", hit);
  // 不透明部分に乗ったらクリックを受け付け、離れたら再び透過する
  await invoke("set_click_through", { ignore: !hit });
}

// クリックスルー中は WebView にマウスイベントが届かないため、
// Rust 側がポーリングしたカーソル座標を受け取って判定する
await listen<CursorPayload>("cursor-moved", (e) => {
  update(e.payload.x, e.payload.y);
});

// Rust 側のランダム移動に合わせて、歩くアニメーションと向きを切り替える
await listen<MovePayload>("mascot-move", (e) => {
  mascot.classList.toggle("walking", e.payload.moving);
  if (e.payload.dir !== 0) {
    mascot.classList.toggle("facing-left", e.payload.dir < 0);
  }
});

// 押してそのまま動かしたらドラッグ、その場で離したらクリックとして扱う
const DRAG_THRESHOLD = 4;
const HAPPY_DURATION_MS = 1000;
let pressAt: { x: number; y: number } | null = null;
let happyTimer: number | undefined;

function rejoice() {
  // 連打されたらアニメーションを最初からやり直す
  mascot.classList.remove("happy");
  void mascot.getBoundingClientRect();
  mascot.classList.add("happy");
  clearTimeout(happyTimer);
  happyTimer = window.setTimeout(() => mascot.classList.remove("happy"), HAPPY_DURATION_MS);
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

const DOUBLE_CLICK_WAIT_MS = 250;
const SPIN_DURATION_MS = 800;
let clickTimer: number | undefined;
let spinTimer: number | undefined;

function spin() {
  mascot.classList.remove("happy", "spinning");
  void mascot.getBoundingClientRect();
  mascot.classList.add("spinning");
  clearTimeout(spinTimer);
  spinTimer = window.setTimeout(() => mascot.classList.remove("spinning"), SPIN_DURATION_MS);
}

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
