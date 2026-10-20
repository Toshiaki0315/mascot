// 16×16 のドット絵スプライト
//   . 透明  O 輪郭  B 体  E 目  e 目の上半分（まばたきで消える）  P ほっぺ
const BODY = [
  "..OO........OO..",
  ".OBBO......OBBO.",
  ".OBBBO....OBBBO.",
  ".OBBBBOOOOBBBBO.",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBBeeBBBBBBeeBBO",
  "OBBEEBBBBBBEEBBO",
  "OBPPBBBBBBBBPPBO",
  "OBBBBBBOOBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  ".OBBBBBBBBBBBBO.",
  "..OOBBBOOBBBOO..",
];

// 足は 2 コマ（12 行目から下に重ねる）
const LEGS_A = [
  "...OBBO..OBBO...",
  "...OOOO..OOOO...",
];
const LEGS_B = [
  "..OBBO....OBBO..",
  "..OOOO....OOOO..",
];
const LEGS_TOP = 13;

// 喜んでいる時の顔（通常の目と口の上に重ねる）。H はハート
const HAPPY_FACE = [
  "..BEEB....BEEB..",
  "..EBBE....EBBE..",
  "................",
  "......OEEO......",
  ".......OO.......",
];
const HAPPY_FACE_TOP = 6;
const HEART = [
  "......HH.HH.....",
  "......HHHHH.....",
  ".......HHH......",
  "........H.......",
];

const SHADOW = "...SSSSSSSSSS...";
const SHADOW_ROW = 15;

const SVG_NS = "http://www.w3.org/2000/svg";

function group(className: string): SVGGElement {
  const g = document.createElementNS(SVG_NS, "g");
  g.setAttribute("class", className);
  return g;
}

// 同じ色が横に続くドットは 1 つの矩形にまとめる
function drawRows(g: SVGGElement, rows: string[], top: number, hit: boolean) {
  rows.forEach((row, i) => {
    let x = 0;
    while (x < row.length) {
      const ch = row[x];
      let end = x + 1;
      while (end < row.length && row[end] === ch) end++;
      if (ch !== ".") {
        const rect = document.createElementNS(SVG_NS, "rect");
        rect.setAttribute("x", String(x));
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

export function renderSprite(svg: SVGSVGElement) {
  const root = group("");
  root.id = "mascot-body";

  const shadow = group("shadow");
  drawRows(shadow, [SHADOW], SHADOW_ROW, false);

  const bounce = group("bounce");
  const legsA = group("legs-a");
  const legsB = group("legs-b");
  drawRows(bounce, BODY, 0, true);
  drawRows(legsA, LEGS_A, LEGS_TOP, true);
  drawRows(legsB, LEGS_B, LEGS_TOP, true);
  const happy = group("face-happy");
  drawRows(happy, HAPPY_FACE, HAPPY_FACE_TOP, false);
  const heart = group("heart");
  drawRows(heart, HEART, 0, false);
  bounce.append(legsA, legsB, happy, heart);

  // 回転は跳ねる動きと transform が競合しないよう、外側のグループで行う
  const spin = group("spin");
  spin.append(bounce);

  root.append(shadow, spin);
  svg.replaceChildren(root);
}
