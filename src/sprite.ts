// 16×16 のドット絵スプライト
//   . 透明  O 輪郭  B 体  D / K 模様（三毛の茶・黒、いぬの耳）  E 目  e 目の上半分（まばたきで消える）
//   C 胸の白（黒ネコ）  Q ピンクの耳・鼻（白ネコ）  J 舌（いぬ）  F Y 首輪の赤・鈴の黄（三毛）
//   R 鼻・耳の内側（くらにゃん）  W 口（くらにゃん）  F Y G T M くらにゃんのスカーフの赤・黄・緑・青緑・紫  P ほっぺ  H ハート  Z 寝息の「Z」
// 色はキャラクターごとに styles.css の CSS 変数で決まる。
// 目・表情は体の上に重ねる部品にして、CSS で切り替える

import { fitBox, type CustomImage } from "./custom-image.ts";

export const CHARACTERS = ["tora", "kuro", "shiro", "mike", "inu", "kuranyan", "custom"] as const;
export type Character = (typeof CHARACTERS)[number];

// 「じぶんの絵」は、ドット絵ではなく、選んだ画像をそのまま置く（画像は custom-image.ts）。
// 目や口の部品は付けない（跳ねる・歩く・眠る・喜ぶの動きと、Z やハートは、ほかと同じ）

const CAT_BODY = [
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

// 土台のドット絵に、(行, 列, 文字列) の順で色を重ねる（"." は何もしない）
function paint(body: string[], edits: [number, number, string][]): string[] {
  const rows = body.map((row) => row.split(""));
  for (const [r, c, s] of edits) {
    [...s].forEach((ch, i) => {
      if (ch !== ".") rows[r][c + i] = ch;
    });
  }
  return rows.map((row) => row.join(""));
}

// 茶トラ：額の M 字の縞と、ほおの縞
const TORA_BODY = paint(CAT_BODY, [
  [4, 6, "D..D"],
  [5, 7, "DD"],
  [7, 1, "DD"],
  [7, 13, "DD"],
]);

// 黒ネコ：胸の白い毛（タキシード）
const KURO_BODY = paint(CAT_BODY, [
  [11, 6, "CCCC"],
  [12, 7, "CC"],
]);

// 白ネコ：ピンクの耳と鼻
const SHIRO_BODY = paint(CAT_BODY, [
  [2, 2, "Q"],
  [2, 13, "Q"],
  [8, 7, "QQ"],
]);

// 三毛：左耳から額に茶、右耳に黒のぶち
const MIKE_BODY = [
  "..OO........OO..",
  ".ODDO......OKKO.",
  ".ODDDO....OKKKO.",
  ".ODDDDOOOOBKKKO.",
  "OBDDDBBBBBBBKKBO",
  "OBBDBBBBBBBBBKBO",
  "OBBBBBBBBBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  "OBPPBBBBBBBBPPBO",
  "OBBBBBBOOBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  ".OBBBBBBBBBBBBO.",
  "..OOBBBOOBBBOO..",
];

// いぬ：丸い頭と垂れ耳
const INU_BODY = [
  "................",
  "....OOOOOOOO....",
  "...OBBBBBBBBO...",
  "..ODOBBBBBBODO..",
  ".ODDOBBBBBBODDO.",
  "ODDDBBBBBBBBDDDO",
  "ODDBBBBBBBBBBDDO",
  "ODDBBBBBBBBBBDDO",
  "ODPPBBBBBBBBPPDO",
  "OBBBBBBOOBBBBBBO",
  "OBBBBBBBBBBBBBBO",
  ".OBBBBBBBBBBBBO.",
  "..OOBBBOOBBBOO..",
];

// くらにゃん：ハチワレ風のおでこ（白いくさび）に茶と黒のぶち、大きなほっぺ、虹色のスカーフ。
// 鼻と口は目と同じく別の部品（下の KURANYAN_*）で、向きや表情で切り替える
const KURANYAN_BODY = [
  "..OO........OO..",
  ".ODDO......OKKO.",
  ".ODRDO....OKRKO.",
  ".ODDDDOOOOKKKKO.",
  ".ODDDDDBBKKKKKO.",
  ".ODDDDBBBBKKKKO.",
  ".ODDBBBBBBBBKKO.",
  ".OBBBBBBBBBBBBO.",
  "OOPPPBBBBBBPPPOO",
  ".OPPPBBBBBBPPPO.",
  "OOBBBBBBBBBBBBOO",
  ".OFYGTFYGTFYGTO.",
  "..OOGTFOOYGTMM..",
  ".............T..",
  ".............T..",
];

// くらにゃんの目は 1 ドット（まばたきで消える）
const KURANYAN_EYES = [
  "................",
  "....e......e....",
];
// 鼻（8 行目）。正面は 2 ドットで中央、向いたときは 1 ドットで向いた側にずれる
const KURANYAN_NOSE_FRONT = [".......RR......."];
const KURANYAN_NOSE_LEFT = [".......R........"];
const KURANYAN_NOSE_RIGHT = ["........R......."];
const KURANYAN_NOSE_TOP = 8;
// 口（9 行目、輪郭なしの赤い口）。表情の間は隠す
const KURANYAN_MOUTH = ["......WWWW......"];
const KURANYAN_MOUTH_TOP = 9;
// くらにゃんの表情（FACE_TOP 行目から）。ふつうのネコ用のものはスカーフや鼻に重なるので、専用にしている
const KURANYAN_HAPPY_FACE = [
  "....E......E....",
  "...E.E....E.E...",
  "................",
  ".....ORRRRO.....",
];
const KURANYAN_SLEEP_FACE = [
  "................",
  "...EEE....EEE...",
  "................",
  ".......WW.......",
];
const KURANYAN_YAWN_FACE = [
  "................",
  "...EEE....EEE...",
  "................",
  ".....ORRRRO.....",
  "......OOOO......",
];
const KURANYAN_FLYING_FACE = [
  "....E......E....",
  "....E......E....",
  "................",
  ".......OO.......",
];

const BODIES: Record<Character, string[]> = {
  tora: TORA_BODY,
  kuro: KURO_BODY,
  shiro: SHIRO_BODY,
  // 三毛は赤い首輪と鈴
  mike: paint(MIKE_BODY, [
    [11, 2, "FFFFFFFFFFFF"],
    [12, 7, "YY"],
  ]),
  // いぬは舌ぺろ
  inu: paint(INU_BODY, [
    [10, 7, "JJ"],
    [11, 7, "JJ"],
  ]),
  kuranyan: KURANYAN_BODY,
  // 画像をそのまま置くので、ドット絵はない
  custom: [],
};

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

// 表情。表示中は通常の目を隠す
const HAPPY_FACE = [
  "...EE......EE...",
  "..E..E....E..E..",
  "................",
  "......OEEO......",
  ".......OO.......",
];
const SLEEP_FACE = [
  "................",
  "...EE......EE...",
];
const YAWN_FACE = [
  "................",
  "...EE......EE...",
  "................",
  "......OEEO......",
  "......OEEO......",
  ".......OO.......",
];
// 投げられた・落ちている時のびっくり顔
const FLYING_FACE = [
  "...EE......EE...",
  "...EE......EE...",
  "...EE......EE...",
  "................",
  ".......EE.......",
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

/// 選んだ画像を、SVG の中に置く（縦横の比を保って、下の真ん中に）
function customImageElement(image: CustomImage): SVGImageElement {
  const box = fitBox(image.width, image.height);
  const el = document.createElementNS(SVG_NS, "image");
  el.setAttribute("href", image.url);
  el.setAttribute("x", String(box.x));
  el.setAttribute("y", String(box.y));
  el.setAttribute("width", String(box.w));
  el.setAttribute("height", String(box.h));
  el.setAttribute("preserveAspectRatio", "none");
  // 当たり判定は、画像の透明でない部分だけ（main.ts が、画像のピクセルを見て決める）
  el.setAttribute("class", "custom-image hit");
  return el;
}

export function renderSprite(svg: SVGSVGElement, character: Character = "tora", custom?: CustomImage) {
  for (const c of CHARACTERS) {
    svg.classList.toggle(`char-${c}`, c === character);
  }

  const root = group("");
  root.id = "mascot-body";

  const shadow = part("shadow", [SHADOW], SHADOW_ROW);

  const bounce = group("bounce");
  const zzz = group("zzz");
  zzz.append(part("z-small", Z_SMALL, -2, false, 7), part("z-large", Z_LARGE, -3, false, 6));

  if (character === "custom") {
    // 目や口の部品はない。画像と、ハートと Z だけ
    if (custom) bounce.append(customImageElement(custom));
    bounce.append(part("heart", HEART, 0), zzz);
  } else {
    drawRows(bounce, BODIES[character], 0, true);
    const kuranyan = character === "kuranyan";
    bounce.append(
      part("eyes", kuranyan ? KURANYAN_EYES : EYES, FACE_TOP),
      ...(kuranyan
        ? [
            part("nose nose-front", KURANYAN_NOSE_FRONT, KURANYAN_NOSE_TOP),
            part("nose nose-left", KURANYAN_NOSE_LEFT, KURANYAN_NOSE_TOP),
            part("nose nose-right", KURANYAN_NOSE_RIGHT, KURANYAN_NOSE_TOP),
            part("mouth", KURANYAN_MOUTH, KURANYAN_MOUTH_TOP),
          ]
        : []),
      part("legs-a", LEGS_A, LEGS_TOP, true),
      part("legs-b", LEGS_B, LEGS_TOP, true),
      part("face-happy", kuranyan ? KURANYAN_HAPPY_FACE : HAPPY_FACE, FACE_TOP),
      part("face-sleep", kuranyan ? KURANYAN_SLEEP_FACE : SLEEP_FACE, FACE_TOP),
      part("face-yawn", kuranyan ? KURANYAN_YAWN_FACE : YAWN_FACE, FACE_TOP),
      part("face-flying", kuranyan ? KURANYAN_FLYING_FACE : FLYING_FACE, FACE_TOP),
      part("heart", HEART, 0),
      zzz,
    );
  }

  // 回転は跳ねる動きと transform が競合しないよう、外側のグループで行う
  const spin = group("spin");
  spin.append(bounce);

  root.append(shadow, spin);
  svg.replaceChildren(root);
}

// 描画に使うすべてのドット絵（名前 → 行）。確認用
function allSprites(): Record<string, string[]> {
  return {
    ...BODIES,
    EYES,
    KURANYAN_EYES,
    KURANYAN_NOSE_FRONT,
    KURANYAN_NOSE_LEFT,
    KURANYAN_NOSE_RIGHT,
    KURANYAN_MOUTH,
    KURANYAN_HAPPY_FACE,
    KURANYAN_SLEEP_FACE,
    KURANYAN_YAWN_FACE,
    KURANYAN_FLYING_FACE,
    LEGS_A,
    LEGS_B,
    HAPPY_FACE,
    SLEEP_FACE,
    YAWN_FACE,
    FLYING_FACE,
    HEART,
    SHADOW: [SHADOW],
  };
}

// 開発時の確認用：すべての行が 16 ドット幅か
export function validateSprites(): string[] {
  const errors: string[] = [];
  for (const [name, rows] of Object.entries(allSprites())) {
    rows.forEach((row, i) => {
      if (row.length !== 16) errors.push(`${name}[${i}] is ${row.length} wide`);
    });
  }
  return errors;
}

// 確認用：ドット絵が使う色の文字（CSS の .px-X と対応する。e は目の上半分で E と同じ色）
export function usedColors(): Set<string> {
  const colors = new Set<string>();
  for (const rows of Object.values(allSprites())) {
    for (const ch of rows.join("")) {
      if (ch !== ".") colors.add(ch === "e" ? "E" : ch);
    }
  }
  return colors;
}
