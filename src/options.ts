import { invoke } from "@tauri-apps/api/core";

// 各機能の設定（音を鳴らすか、メニューバーに残り時間を出すか、タイマーの秒読み）。
// 画面のあちこちにある data-option の付いた入力欄（チェックボックスと選択）を、
// 同じ名前の設定につなぐ。変えたらすぐ保存する

type Options = {
  reminder_sound: boolean;
  timer_sound: boolean;
  pomodoro_sound: boolean;
  menubar_timer: boolean;
  menubar_pomodoro: boolean;
  countdown_mode: "random" | "always" | "never";
};

const inputs = [...document.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-option]")];
let options = await invoke<Options>("get_options");

function show() {
  for (const el of inputs) {
    const value = options[el.dataset.option as keyof Options];
    if (el instanceof HTMLInputElement) el.checked = value === true;
    else el.value = String(value);
  }
}

for (const el of inputs) {
  el.addEventListener("change", async () => {
    const name = el.dataset.option as keyof Options;
    const value = el instanceof HTMLInputElement ? el.checked : el.value;
    options = { ...options, [name]: value };
    await invoke("set_options", { options });
  });
}

show();
