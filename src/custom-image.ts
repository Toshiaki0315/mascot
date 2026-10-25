// 「じぶんの絵」：自分で選んだ画像をキャラクターにする。DOM に触れない計算の部分

/// マスコットの SVG の座標（16×16）のうち、画像を置ける範囲。いちばん下の 1 行は、影のために空ける
export const BOX_W = 16;
export const BOX_H = 15;
/// 渡す画像の長いほうの辺の最大（ピクセル）。2 倍の大きさで表示しても、ぼやけすぎない大きさ
export const MAX_SIDE = 256;
/// これより透明でない点を、キャラクターの上とみなす（0〜255。うすい縁には反応しない）
export const OPAQUE_ALPHA = 24;

export type CustomImage = {
  /// `data:image/png;base64,…`
  url: string;
  width: number;
  height: number;
  /// 画像の RGBA（クリックの判定に使う。幅 × 高さ × 4 バイト）。描くだけなら要らない
  pixels?: Uint8ClampedArray;
};

export type Box = { x: number; y: number; w: number; h: number };

/// 縦横の比を保って、範囲（16×15）に収まる最大の大きさにして、下の真ん中に置く
export function fitBox(width: number, height: number): Box {
  if (width <= 0 || height <= 0) return { x: 0, y: 0, w: 0, h: 0 };
  const scale = Math.min(BOX_W / width, BOX_H / height);
  const w = width * scale;
  const h = height * scale;
  return { x: (BOX_W - w) / 2, y: BOX_H - h, w, h };
}

/// 長いほうの辺が `max` を超えるときだけ、比を保って小さくした大きさ（整数）
export function scaleDown(width: number, height: number, max = MAX_SIDE): { width: number; height: number } {
  const longest = Math.max(width, height);
  if (longest <= max) return { width, height };
  const scale = max / longest;
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}

/// SVG の座標（16×16）の点が、画像のどのピクセルにあたるか。画像の外なら null
export function toImagePixel(
  ux: number,
  uy: number,
  box: Box,
  width: number,
  height: number,
): { x: number; y: number } | null {
  if (box.w <= 0 || box.h <= 0) return null;
  const fx = (ux - box.x) / box.w;
  const fy = (uy - box.y) / box.h;
  if (fx < 0 || fy < 0 || fx >= 1 || fy >= 1) return null;
  return { x: Math.floor(fx * width), y: Math.floor(fy * height) };
}

/// 画像の（px, py）が、透明でないか
export function isOpaqueAt(pixels: Uint8ClampedArray, width: number, height: number, px: number, py: number): boolean {
  if (px < 0 || py < 0 || px >= width || py >= height) return false;
  return pixels[(py * width + px) * 4 + 3] >= OPAQUE_ALPHA;
}

/// SVG の座標の点が、画像の透明でない部分の上か
export function hitsCustomImage(image: CustomImage, ux: number, uy: number): boolean {
  if (!image.pixels) return false;
  const p = toImagePixel(ux, uy, fitBox(image.width, image.height), image.width, image.height);
  return p !== null && isOpaqueAt(image.pixels, image.width, image.height, p.x, p.y);
}
