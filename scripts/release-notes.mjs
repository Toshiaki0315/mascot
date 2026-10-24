// 使い方: node scripts/release-notes.mjs <タグ> [前のタグ]
// リリースノートの下書きを作る。前のタグからのコミットを「新機能」「修正」「変更」に分け、
// インストール方法と注意のひな形と一緒に出す。コミットの文は英語なので、公開する前に整える
import { execFileSync } from "node:child_process";

const tag = process.argv[2];
if (!tag) {
  console.error("使い方: node scripts/release-notes.mjs <tag> [previous tag]");
  process.exit(1);
}
const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
const version = tag.replace(/^v/, "");
// 前のタグ：指定がなければ、このタグより前で一番近いタグ
const previous = process.argv[3] ?? git("describe", "--tags", "--abbrev=0", `${tag}^`);

const SEP = "\u001e";
const commits = git("log", "--reverse", `--format=%s%n%b${SEP}`, `${previous}..${tag}`)
  .split(SEP)
  .map((c) => c.trim())
  .filter(Boolean)
  .map((c) => {
    const [subject, ...body] = c.split("\n");
    // 本文のうち、箇条書きの行だけを細目として残す（署名の行などは除く）
    const details = body.map((l) => l.trim()).filter((l) => l.startsWith("- "));
    return { subject: subject.trim(), details };
  })
  .filter((c) => !/^Bump version/i.test(c.subject) && !/^Merge /.test(c.subject));

const groups = { 新機能: [], 修正: [], 変更: [] };
for (const c of commits) {
  if (/^(Fix|Correct|Repair)\b/i.test(c.subject)) groups["修正"].push(c);
  else if (/^(Add|Support|Allow|Introduce)\b/i.test(c.subject)) groups["新機能"].push(c);
  else groups["変更"].push(c);
}

const lines = [];
for (const [title, items] of Object.entries(groups)) {
  if (items.length === 0) continue;
  lines.push(`## ${title}`);
  for (const c of items) {
    lines.push(`- ${c.subject}`);
    for (const d of c.details) lines.push(`  ${d}`);
  }
  lines.push("");
}
if (commits.length === 0) lines.push("## 変更", "- 細かな修正", "");

lines.push(
  "## インストール",
  `1. \`Desktop-Mascot_${version}_aarch64.dmg\` をダウンロードして開きます`,
  "2. `Desktop Mascot.app` を「アプリケーション」フォルダへドラッグします（以前の版は上書きされます）",
  "",
  "## 注意",
  "- **Apple Silicon (M1 以降) 専用**です",
  "- Apple Developer ID による署名・公証はしていないため、初回起動時に macOS にブロックされます。「システム設定 → プライバシーとセキュリティ」の「このまま開く」から起動してください。うまくいかない場合はターミナルで次を実行してください:",
  "  ```bash",
  '  xattr -dr com.apple.quarantine "/Applications/Desktop Mascot.app"',
  "  ```",
  "",
  `${previous} からの変更点の一覧は [${previous}...${tag}](https://github.com/Toshiaki0315/mascot/compare/${previous}...${tag}) を参照してください。`,
);
console.log(lines.join("\n"));
