// 16×16 のドット絵スプライト
//   . 透明  O 輪郭  B 体  E 目  e 目の上半分（まばたきで消える）  P ほっぺ
//   H ハート  Z 寝息の「Z」
// 目・表情は体の上に重ねる部品にして、CSS で切り替える
const BODY = [
  "..OO........OO..",
  ".OBBO......OBBO.",
  ".OBBBO....OBBBO.",
  ".OBBBBOOOOBBBBO.",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBPPBBBBBBBBPPBO",
  "OBBBBBBOOBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  ".OBBBBBBBBBBBBO.",
  "..OOBBBOOBBBOO..",
];

// 通常の目。カーソルや進行方向に合わせて左右に 1 ドットずらす
const EYES = [
  "...ee......ee...",
  "...EE......EE...",
];
const FACE_TOP = 6;

// 足は 2 コマ（13 行目から下に重ねる）
const LEGS_A = [
  "...OBBO..OBBO...",
  "...OOOO..OOOO...",
];
const LEGS_B = [
  "..OBBO....OBBO..",
  "..OOOO....OOOO..",
];
const LEGS_TOP = 13;

// 表情（目の位置をずらしても隠れるよう、左右に 1 ドット広く塗る）
const HAPPY_FACE = [
  "..BEEB....BEEB..",
  "..EBBE....EBBE..",
  "................",
  "......OEEO......",
  ".......OO.......",
];
const SLEEP_FACE = [
  "..BBBB....BBBB..",
  "..BEEB....BEEB..",
];
const YAWN_FACE = [
  "..BBBB....BBBB..",
  "..BEEB....BEEB..",
  "................",
  "......OEEO......",
  "......OEEO......",
  ".......OO.......",
];

const HEART = [
  "......HH.HH.....",
  "......HHHHH.....",
  ".......HHH......",
  "........H.......",
];

// 寝息の Z（小と大を交互に出す）。耳と耳の間から上の余白にかけて描く
const Z_SMALL = ["ZZZZ", "..Z.", ".Z..", "ZZZZ"];
const Z_LARGE = ["ZZZZZ", "...Z.", "..Z..", ".Z...", "ZZZZZ"];

const SHADOW = "...SSSSSSSSSS...";
const SHADOW_ROW = 15;

const SVG_NS = "http://www.w3.org/2000/svg";

function group(className: string): SVGGElement {
  const g = document.createElementNS(SVG_NS, "g");
  g.setAttribute("class", className);
  return g;
}

// 同じ色が横に続くドットは 1 つの矩形にまとめる
function drawRows(
  g: SVGGElement,
  rows: string[],
  top: number,
  hit: boolean,
  left = 0,
) {
  rows.forEach((row, i) => {
    let x = 0;
    while (x < row.length) {
      const ch = row[x];
      let end = x + 1;
      while (end < row.length && row[end] === ch) end++;
      if (ch !== ".") {
        const rect = document.createElementNS(SVG_NS, "rect");
        rect.setAttribute("x", String(left + x));
        rect.setAttribute("y", String(top + i));
        rect.setAttribute("width", String(end - x));
        rect.setAttribute("height", "1");
        const color = ch === "e" ? "E" : ch;
        const classes = [`px-${color}`];
        if (ch === "e") classes.push("eye-top");
        if (hit) classes.push("hit");
        rect.setAttribute("class", classes.join(" "));
        g.appendChild(rect);
      }
      x = end;
    }
  });
}

function part(className: string, rows: string[], top: number, hit = false, left = 0) {
  const g = group(className);
  drawRows(g, rows, top, hit, left);
  return g;
}

export function renderSprite(svg: SVGSVGElement) {
  const root = group("");
  root.id = "mascot-body";

  const shadow = part("shadow", [SHADOW], SHADOW_ROW);

  const bounce = group("bounce");
  drawRows(bounce, BODY, 0, true);
  const zzz = group("zzz");
  zzz.append(part("z-small", Z_SMALL, -2, false, 7), part("z-large", Z_LARGE, -3, false, 6));
  bounce.append(
    part("eyes", EYES, FACE_TOP),
    part("legs-a", LEGS_A, LEGS_TOP, true),
    part("legs-b", LEGS_B, LEGS_TOP, true),
    part("face-happy", HAPPY_FACE, FACE_TOP),
    part("face-sleep", SLEEP_FACE, FACE_TOP),
    part("face-yawn", YAWN_FACE, FACE_TOP),
    part("heart", HEART, 0),
    zzz,
  );

  // 回転は跳ねる動きと transform が競合しないよう、外側のグループで行う
  const spin = group("spin");
  spin.append(bounce);

  root.append(shadow, spin);
  svg.replaceChildren(root);
}
