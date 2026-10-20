// 使い方: node scripts/gen-icon.mjs <出力先.svg>（通常は make icon から呼ばれる）
// ドット絵のネコ（喜び顔 + ハート）を macOS 風の角丸アイコンに配置した SVG を生成する
import { writeFileSync } from "node:fs";

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
