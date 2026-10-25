import { MAX_SIDE, scaleDown, type CustomImage } from "./custom-image";

// 「じぶんの絵」の、ブラウザの機能（画像の読み込み・canvas）を使う部分

/// `data:` の画像を読んで、大きさと、ピクセル（クリックの判定用）を取り出す
export async function decodeImage(url: string): Promise<CustomImage> {
  const img = new Image();
  img.src = url;
  await img.decode();
  const width = img.naturalWidth;
  const height = img.naturalHeight;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("画像を調べられませんでした");
  ctx.drawImage(img, 0, 0);
  return { url, width, height, pixels: ctx.getImageData(0, 0, width, height).data };
}

/// ファイルを `data:` の文字列として読む。
/// （`blob:` の URL は、製品版の CSP（img-src）で止められるので、使わない）
function readAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("ファイルを読み込めませんでした"));
    reader.readAsDataURL(file);
  });
}

/// 選んだ画像のファイルを、長いほうの辺が MAX_SIDE 以下の PNG にして、`data:` の文字列で返す
/// （透明な部分は、そのまま透明で残る）
export async function prepareImage(file: File): Promise<string> {
  const img = new Image();
  img.src = await readAsDataUrl(file);
  try {
    await img.decode();
  } catch {
    throw new Error("画像として読み込めませんでした（PNG、JPEG、GIF、WebP などを選んでください）");
  }
  const { width, height } = scaleDown(img.naturalWidth, img.naturalHeight, MAX_SIDE);
  if (width === 0 || height === 0) throw new Error("画像の大きさが読み取れませんでした");
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("画像を小さくできませんでした");
  ctx.drawImage(img, 0, 0, width, height);
  return canvas.toDataURL("image/png");
}
