import { invoke } from "@tauri-apps/api/core";

// 「一般」の設定（歩き回る・歩く速さ・大きさ・おしゃべり・全画面アプリの上では隠れる・ログイン時に起動）。
// data-general の付いた入力欄を、同じ名前の設定につなぐ。変えたらすぐ保存して、反映する

type General = {
  walking: boolean;
  speed: "slow" | "normal" | "fast";
  scale: number;
  chatty: boolean;
  hide_on_fullscreen: boolean;
  autostart: boolean;
};

const inputs = [...document.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-general]")];
const errorEl = document.querySelector<HTMLElement>("#general-error")!;
let general = await invoke<General>("get_general");

function show() {
  for (const el of inputs) {
    const value = general[el.dataset.general as keyof General];
    if (el instanceof HTMLInputElement) el.checked = value === true;
    else el.value = String(value);
  }
}

function read(el: HTMLInputElement | HTMLSelectElement): boolean | number | string {
  if (el instanceof HTMLInputElement) return el.checked;
  // 大きさは数、速さは文字
  return el.dataset.general === "scale" ? Number(el.value) : el.value;
}

for (const el of inputs) {
  el.addEventListener("change", async () => {
    errorEl.textContent = "";
    const next = { ...general, [el.dataset.general as string]: read(el) };
    try {
      // 反映した実際の状態が返る（ログイン時の起動は、登録に失敗すると元に戻る）
      general = await invoke<General>("set_general", { general: next });
    } catch (err) {
      errorEl.textContent = String(err);
      general = await invoke<General>("get_general");
    }
    show();
  });
}

show();
