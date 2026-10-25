import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BOX_H,
  BOX_W,
  fitBox,
  hitsCustomImage,
  isOpaqueAt,
  MAX_SIDE,
  OPAQUE_ALPHA,
  scaleDown,
  toImagePixel,
  type CustomImage,
} from "./custom-image.ts";

test("正方形の画像は、範囲いっぱい（高さに合わせて）下の真ん中に置く", () => {
  const box = fitBox(100, 100);
  assert.equal(box.h, BOX_H);
  assert.equal(box.w, BOX_H);
  assert.equal(box.y, 0);
  assert.equal(box.x, (BOX_W - BOX_H) / 2);
});

test("横長の画像は、幅に合わせて小さくし、下に置く", () => {
  const box = fitBox(200, 100);
  assert.equal(box.w, BOX_W);
  assert.equal(box.h, BOX_W / 2);
  assert.equal(box.x, 0);
  // 足元が、いつも下の端（影の 1 行の上）にそろう
  assert.equal(box.y + box.h, BOX_H);
});

test("縦長の画像は、高さに合わせる", () => {
  const box = fitBox(50, 200);
  assert.equal(box.h, BOX_H);
  assert.equal(box.w, BOX_H / 4);
  assert.equal(box.y + box.h, BOX_H);
});

test("大きさのない画像は、何も置かない", () => {
  assert.deepEqual(fitBox(0, 10), { x: 0, y: 0, w: 0, h: 0 });
  assert.deepEqual(fitBox(10, 0), { x: 0, y: 0, w: 0, h: 0 });
});

test("小さくするのは、長いほうの辺が上限を超えるときだけ", () => {
  assert.deepEqual(scaleDown(100, 50), { width: 100, height: 50 });
  assert.deepEqual(scaleDown(MAX_SIDE, MAX_SIDE), { width: MAX_SIDE, height: MAX_SIDE });
  assert.deepEqual(scaleDown(1024, 512), { width: MAX_SIDE, height: MAX_SIDE / 2 });
  assert.deepEqual(scaleDown(512, 2048), { width: 64, height: MAX_SIDE });
});

test("極端に細長い画像でも、辺は 1 ピクセル未満にならない", () => {
  assert.deepEqual(scaleDown(10_000, 1), { width: MAX_SIDE, height: 1 });
});

test("SVG の点を画像のピクセルに直す（範囲の外は null）", () => {
  const box = fitBox(100, 100);
  const left = box.x;
  assert.deepEqual(toImagePixel(left, 0, box, 100, 100), { x: 0, y: 0 });
  assert.deepEqual(toImagePixel(left + box.w - 0.001, BOX_H - 0.001, box, 100, 100), { x: 99, y: 99 });
  assert.equal(toImagePixel(left - 0.1, 5, box, 100, 100), null);
  assert.equal(toImagePixel(left + box.w, 5, box, 100, 100), null);
  assert.equal(toImagePixel(left + 1, -0.1, box, 100, 100), null);
  assert.equal(toImagePixel(left + 1, BOX_H, box, 100, 100), null);
});

/// 幅 w × 高さ h で、指定したピクセルだけ不透明な画像のピクセル列
function pixelsWith(w: number, h: number, opaque: [number, number][], alpha = 255): Uint8ClampedArray {
  const data = new Uint8ClampedArray(w * h * 4);
  for (const [x, y] of opaque) data[(y * w + x) * 4 + 3] = alpha;
  return data;
}

test("透明でない点だけが、当たりになる（うすすぎる縁は当たらない）", () => {
  assert.equal(isOpaqueAt(pixelsWith(4, 4, [[1, 2]]), 4, 4, 1, 2), true);
  assert.equal(isOpaqueAt(pixelsWith(4, 4, [[1, 2]]), 4, 4, 2, 2), false);
  assert.equal(isOpaqueAt(pixelsWith(4, 4, [[1, 2]], OPAQUE_ALPHA), 4, 4, 1, 2), true);
  assert.equal(isOpaqueAt(pixelsWith(4, 4, [[1, 2]], OPAQUE_ALPHA - 1), 4, 4, 1, 2), false);
});

test("画像の外のピクセルは、当たらない", () => {
  const data = pixelsWith(4, 4, [[0, 0]]);
  assert.equal(isOpaqueAt(data, 4, 4, -1, 0), false);
  assert.equal(isOpaqueAt(data, 4, 4, 4, 0), false);
  assert.equal(isOpaqueAt(data, 4, 4, 0, 4), false);
});

test("画像全体での判定：真ん中だけ不透明な画像", () => {
  // 10×10 の画像の、真ん中の 4×4 だけが不透明
  const opaque: [number, number][] = [];
  for (let y = 3; y < 7; y++) for (let x = 3; x < 7; x++) opaque.push([x, y]);
  const image: CustomImage = {
    url: "data:image/png;base64,AAAA",
    width: 10,
    height: 10,
    pixels: pixelsWith(10, 10, opaque),
  };
  const box = fitBox(10, 10);
  const centerX = box.x + box.w / 2;
  const centerY = box.y + box.h / 2;
  assert.equal(hitsCustomImage(image, centerX, centerY), true);
  // 四隅は、画像の中でも透明なので当たらない
  assert.equal(hitsCustomImage(image, box.x + 0.1, box.y + 0.1), false);
  // 画像の外
  assert.equal(hitsCustomImage(image, 0, 0), false);
});

test("ピクセルを持たない画像は、当たらない", () => {
  const image: CustomImage = { url: "data:image/png;base64,AAAA", width: 4, height: 4 };
  assert.equal(hitsCustomImage(image, 8, 8), false);
});
