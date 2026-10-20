// 使い方: node scripts/gen-icon.mjs <アプリアイコン.svg> <トレイアイコン.png>（通常は make icon から呼ばれる）
// - アプリアイコン: ドット絵のネコ（喜び顔 + ハート）を macOS 風の角丸アイコンに配置した SVG
// - トレイアイコン: ネコのシルエットを目だけ抜いた白黒の PNG（macOS のテンプレート画像）
import { writeFileSync } from "node:fs";
import { crc32, deflateSync } from "node:zlib";

const SPRITE = [
  "..OO........OO..",
  ".OBBO......OBBO.",
  ".OBBBO....OBBBO.",
  ".OBBBBOOOOBBBBO.",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBBEEBBBBBBEEBBO",
  "OBEBBEBBBBEBBEBO",
  "OBPPBBBBBBBBPPBO",
  "OBBBBBOBBOBBBBBO",
  "OBBBBBBOOBBBBBBO",
  ".OBBBBBBBBBBBBO.",
  "..OOBBBOOBBBOO..",
  "...OBBO..OBBO...",
  "...OOOO..OOOO...",
];
const HEART = ["HH.HH", "HHHHH", ".HHH.", "..H.."];
const COLORS = { O: "#5a3a1f", B: "#ffd27a", E: "#3b2a1a", P: "#ff9a9a", H: "#ff4f7a", S: "rgba(0,0,0,0.22)", W: "#ffffff" };

const DOT = 32;
const rects = [];
function draw(rows, ox, oy, dot = DOT) {
  rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      const ch = row[x];
      let end = x + 1;
      while (end < row.length && row[end] === ch) end++;
      if (ch !== ".") {
        rects.push(`<rect x="${ox + x * dot}" y="${oy + y * dot}" width="${(end - x) * dot}" height="${dot}" fill="${COLORS[ch]}"/>`);
      }
      x = end;
    }
  });
}

// 1024 キャンバスに Apple のアイコングリッド（824 角、角丸 185）で背景を置く
const ox = 512 - 8 * DOT;
const oy = 300;
draw(["SSSSSSSSSS"], ox + 3 * DOT, oy + 15 * DOT);
draw(SPRITE, ox, oy);
draw(HEART, 700, 196, 24);
draw([".W.", "WWW", ".W."], 196, 260, 20);
draw([".W.", "WWW", ".W."], 810, 470, 14);

const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" shape-rendering="crispEdges">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#8fd8ff"/>
      <stop offset="1" stop-color="#5b8def"/>
    </linearGradient>
  </defs>
  <rect x="100" y="100" width="824" height="824" rx="185" fill="url(#bg)" shape-rendering="geometricPrecision"/>
  <rect x="100" y="760" width="824" height="164" rx="0" fill="#7ccf6a" clip-path="url(#clip)"/>
  <clipPath id="clip"><rect x="100" y="100" width="824" height="824" rx="185"/></clipPath>
  ${rects.join("\n  ")}
</svg>
`;
writeFileSync(process.argv[2], svg);

// ---- トレイアイコン ----
// 1 ドット = 2px の 32×32。メニューバーでは 16pt で表示される
const TRAY_SPRITE = [
  "..OO........OO..",
  ".OBBO......OBBO.",
  ".OBBBO....OBBBO.",
  ".OBBBBOOOOBBBBO.",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBBEEBBBBBBEEBBO",
  "OBBEEBBBBBBEEBBO",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  ".OBBBBBBBBBBBBO.",
  "..OOBBBOOBBBOO..",
  "...OBBO..OBBO...",
  "...OOOO..OOOO...",
  "................",
];

function encodePng(width, height, rgba) {
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // ビット深度
  ihdr[9] = 6; // RGBA
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    rgba.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

if (process.argv[3]) {
  const SCALE = 2;
  const size = 16 * SCALE;
  const rgba = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const ch = TRAY_SPRITE[Math.floor(y / SCALE)][Math.floor(x / SCALE)];
      // テンプレート画像は透明度だけが使われる。目は抜いて表情を出す
      rgba[(y * size + x) * 4 + 3] = ch === "O" || ch === "B" ? 255 : 0;
    }
  }
  writeFileSync(process.argv[3], encodePng(size, size, rgba));
}
