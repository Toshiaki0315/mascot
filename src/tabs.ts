import { listen } from "@tauri-apps/api/event";

// 見出し（リマインダー / タイマー / メモ / 天気）の切り替え。
// 最初の見出しは URL の ?tab= で決まり、開いたままのウィンドウへは Rust 側が "tools-tab" で知らせる
const TABS = ["reminders", "timers", "notes", "weather"] as const;
type Tab = (typeof TABS)[number];

const buttons = [...document.querySelectorAll<HTMLButtonElement>("[data-tab]")];
const panels = [...document.querySelectorAll<HTMLElement>("[data-panel]")];

function show(tab: string) {
  const name: Tab = TABS.find((t) => t === tab) ?? "reminders";
  for (const b of buttons) b.setAttribute("aria-selected", String(b.dataset.tab === name));
  for (const p of panels) p.hidden = p.dataset.panel !== name;
  document.dispatchEvent(new CustomEvent("tab-shown", { detail: name }));
  try {
    localStorage.setItem("tools-tab", name);
  } catch {
    // 覚えておけなくても、次回は URL の指定どおりに開くだけ
  }
}

for (const b of buttons) b.addEventListener("click", () => show(b.dataset.tab ?? ""));

await listen<string>("tools-tab", (e) => show(e.payload));
show(new URLSearchParams(location.search).get("tab") ?? "reminders");
