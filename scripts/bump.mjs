// 使い方: node scripts/bump.mjs [patch|minor|major|<バージョン>]   （省略時は patch）
// アプリのバージョンを、関係する 5 つのファイルでまとめて上げる：
//   package.json / package-lock.json / src-tauri/Cargo.toml / src-tauri/Cargo.lock / src-tauri/tauri.conf.json
// タグは付けない（コミットして push してから、v<バージョン> のタグを push するとリリースされる）
import { readFileSync, writeFileSync } from "node:fs";

const arg = process.argv[2] ?? "patch";
const read = (p) => readFileSync(p, "utf8");
const current = JSON.parse(read("src-tauri/tauri.conf.json")).version;

function next(version, level) {
  const [major, minor, patch] = version.split(".").map(Number);
  if (level === "major") return `${major + 1}.0.0`;
  if (level === "minor") return `${major}.${minor + 1}.0`;
  if (level === "patch") return `${major}.${minor}.${patch + 1}`;
  return null;
}

const version = next(current, arg) ?? arg.replace(/^v/, "");
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`バージョンの形が正しくありません: ${arg}（patch / minor / major か、1.2.3 の形で指定）`);
  process.exit(1);
}

// 書き換える場所が見つからなければ止める（ファイルの形が変わったのに気づかず、上げ漏れるのを防ぐ）
function replace(path, pattern, replacement) {
  const before = read(path);
  const after = before.replace(pattern, replacement);
  if (after === before) {
    console.error(`${path} のバージョンが見つかりませんでした`);
    process.exit(1);
  }
  writeFileSync(path, after);
}

const json = (key) => new RegExp(`("${key}":\\s*")${current.replace(/\./g, "\\.")}(")`);
replace("src-tauri/tauri.conf.json", json("version"), `$1${version}$2`);
replace("package.json", json("version"), `$1${version}$2`);
// package-lock.json は、先頭と packages[""] の 2 か所
replace(
  "package-lock.json",
  /("name": "desktop-mascot",\s*"version": ")[^"]+(")/g,
  `$1${version}$2`,
);
replace("src-tauri/Cargo.toml", /^(version = ")[^"]+(")/m, `$1${version}$2`);
replace("src-tauri/Cargo.lock", /(name = "desktop-mascot"\nversion = ")[^"]+(")/, `$1${version}$2`);

console.log(`${current} → ${version}`);
