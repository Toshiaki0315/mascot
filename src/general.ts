import { invoke } from "@tauri-apps/api/core";
import { pickImageFile } from "./custom-image";
import { prepareImage } from "./custom-image-dom";

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

// ---- じぶんの絵 ----

const MAX_FILE_BYTES = 20 * 1024 * 1024;
const preview = document.querySelector<HTMLImageElement>("#custom-preview")!;
const empty = document.querySelector<HTMLElement>("#custom-empty")!;
const fileInput = document.querySelector<HTMLInputElement>("#custom-file")!;
const chooseButton = document.querySelector<HTMLButtonElement>("#custom-choose")!;
const clearButton = document.querySelector<HTMLButtonElement>("#custom-clear")!;
const customMessage = document.querySelector<HTMLElement>("#custom-message")!;
const customError = document.querySelector<HTMLElement>("#custom-error")!;

async function showCustom() {
  const url = await invoke<string | null>("get_custom_image");
  preview.hidden = url === null;
  empty.hidden = url !== null;
  if (url) preview.src = url;
  clearButton.disabled = url === null;
}

chooseButton.addEventListener("click", () => fileInput.click());

/// 選んだ（またはドロップされた）画像のファイルを、「じぶんの絵」にする
async function useFile(file: File) {
  customError.textContent = "";
  customMessage.textContent = "";
  if (file.size > MAX_FILE_BYTES) {
    customError.textContent = "ファイルが大きすぎます（20MB まで）";
    return;
  }
  try {
    await invoke("set_custom_image", { dataUrl: await prepareImage(file) });
    customMessage.textContent = "「じぶんの絵」に切り替えました";
    await showCustom();
    // キャラクターが切り替わったので、設定も読み直す
    general = await invoke<General>("get_general");
    show();
  } catch (err) {
    customError.textContent = String(err instanceof Error ? err.message : err);
  }
}

fileInput.addEventListener("change", async () => {
  const file = fileInput.files?.[0];
  // 同じファイルをもう一度選べるよう、選択をもどしておく
  fileInput.value = "";
  if (file) await useFile(file);
});

// ---- ドラッグ＆ドロップ ----
// このウィンドウは、ファイルのドロップを Tauri に取られない設定で開いている（reminder.rs）ので、
// ページが直接受け取れる。枠にドロップされた画像を使う。枠の外にドロップされたときは、
// ブラウザの標準の動き（そのファイルを表示してしまう）にならないよう、何もしない
const dropZone = document.querySelector<HTMLElement>("#custom-drop")!;

for (const type of ["dragover", "drop"] as const) {
  window.addEventListener(type, (e) => e.preventDefault());
}

dropZone.addEventListener("dragenter", () => dropZone.classList.add("dragover"));
dropZone.addEventListener("dragover", (e) => {
  e.preventDefault();
  dropZone.classList.add("dragover");
});
dropZone.addEventListener("dragleave", (e) => {
  // 枠の中の部品に移っただけのときは、そのまま
  if (!dropZone.contains(e.relatedTarget as Node | null)) dropZone.classList.remove("dragover");
});
dropZone.addEventListener("drop", async (e) => {
  e.preventDefault();
  dropZone.classList.remove("dragover");
  const files = e.dataTransfer?.files;
  if (!files || files.length === 0) return;
  const file = pickImageFile(files);
  if (!file) {
    customMessage.textContent = "";
    customError.textContent = "画像のファイルをドロップしてください";
    return;
  }
  await useFile(file);
});

clearButton.addEventListener("click", async () => {
  customError.textContent = "";
  await invoke("clear_custom_image");
  customMessage.textContent = "消しました（茶トラに戻しました）";
  await showCustom();
});

await showCustom();
