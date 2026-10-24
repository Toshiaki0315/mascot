import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { reminderActions } from "./speech";

// 大きなふきだし（専用のウィンドウ）の中身。マスコット側から文章を受け取って描き、
// できあがった大きさを測って Rust 側へ知らせる（Rust 側が大きさを合わせて、マスコットの頭の上に出す）

type Content = { text: string; snooze: boolean; scale: number; chat: boolean };

const wrap = document.querySelector<HTMLElement>("#wrap")!;
const card = document.querySelector<HTMLElement>("#card")!;

function render({ text, snooze, scale, chat }: Content) {
  document.documentElement.style.setProperty("--s", String(scale));
  document.body.classList.toggle("chat", chat);

  const message = document.createElement("div");
  message.textContent = text;
  const actions = document.createElement("div");
  actions.className = "actions";
  const buttons = reminderActions(snooze);
  for (const [action, label] of buttons) {
    const button = document.createElement("span");
    button.className = "action";
    button.dataset.action = action;
    button.textContent = label;
    actions.append(button);
  }
  // ふつうのおしゃべりと、ボタンのないリマインダーには、ボタンの行を付けない
  card.replaceChildren(...(chat || buttons.length === 0 ? [message] : [message, actions]));
}

// 受け取りの準備は待たずに先へ進む（失敗しても、下のクリックの処理は登録されるように）
listen<Content>("bubble-content", (e) => {
  render(e.payload);
  // getBoundingClientRect はレイアウトを済ませてから測る。
  // requestAnimationFrame は、まだ見えていない（隠れている）ウィンドウでは動かないので使わない
  const rect = wrap.getBoundingClientRect();
  void invoke("bubble_resize", { w: Math.ceil(rect.width), h: Math.ceil(rect.height) });
}).catch((err) => console.error("ふきだしの中身を受け取れません", err));

// ボタンの上ならそのボタン、それ以外（文字や枠）なら、ふきだしを閉じる
wrap.addEventListener("click", (e) => {
  const action = (e.target as HTMLElement).closest<HTMLElement>("[data-action]")?.dataset.action;
  void invoke("bubble_action", { action: action === "snooze" ? "snooze" : "close" });
});
