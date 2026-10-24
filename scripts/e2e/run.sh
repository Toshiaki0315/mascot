#!/bin/zsh
# 画面まわりの回帰テスト（make e2e から呼ぶ）。
# 全画面アプリの Space やプレゼンテーションの状態を実際に作って、マスコットとふきだしの出方を確かめる。
#
#   - 作業用のコピーに、識別子を分けたテスト用のアプリを作る（普段のアプリや設定には触れない）
#   - 実行中は約 1 分、画面が切り替わる。本体の画面（内蔵ディスプレイ）を使う
#   - 画面を操作するので CI では動かない。手元の Mac で実行する
#   - 実行中はマウスやキーボードに触らない（ふきだしをクリックすると閉じてしまい、失敗する）
#
# 作業用のフォルダ（ビルド結果を残して 2 回目以降を速くする）は E2E_WORK で変えられる
set -u

ROOT=${0:A:h:h:h}
WORK=${E2E_WORK:-${TMPDIR:-/tmp}/desktop-mascot-e2e}
APP_ID=jp.classmethod.desktop-mascot.e2e
SETTINGS_DIR="$HOME/Library/Application Support/$APP_ID"
mkdir -p "$WORK/bin" "$WORK/src"

failures=0
check() {
  local name=$1 ok=$2 detail=$3
  if [ "$ok" = 1 ]; then
    echo "  ok    $name ($detail)"
  else
    echo "  FAIL  $name ($detail)"
    failures=$((failures + 1))
  fi
}

echo "== 補助ツールをビルド"
for tool in nativefs present windows; do
  swiftc -O "$ROOT/scripts/e2e/$tool.swift" -o "$WORK/bin/$tool" || exit 1
done

echo "== テスト用のアプリをビルド"
rsync -a --delete --exclude node_modules --exclude src-tauri/target --exclude .git --exclude dist "$ROOT/" "$WORK/src/"
ln -sfn "$ROOT/node_modules" "$WORK/src/node_modules"
(cd "$WORK/src" && python3 "$ROOT/scripts/e2e/instrument.py") || exit 1
(cd "$WORK/src" && CARGO_TARGET_DIR="$WORK/target" npx tauri build --bundles app 2>&1 | grep -E "^error|error\[|Finished \`release") || exit 1
APP="$WORK/target/release/bundle/macos/Desktop Mascot.app/Contents/MacOS/desktop-mascot"

# 本体の画面の真ん中あたりに、止まった状態で置く
mkdir -p "$SETTINGS_DIR"
printf '{"position":[800.0,600.0],"visible":true,"walking":false,"hide_on_fullscreen":true,"chatty":true}' \
  > "$SETTINGS_DIR/settings.json"

"$APP" > "$WORK/app.log" 2>&1 &
APP_PID=$!
trap 'kill $APP_PID 2> /dev/null; rm -rf "$SETTINGS_DIR"' EXIT
W=("$WORK/bin/windows" $APP_PID)

echo "== 1. ふきだしが出る"
sleep 12
state=$("${W[@]}" list)
check "マスコットとふきだしが出ている" "$([ "$state" = "mascot=1 bubble=1" ] && echo 1 || echo 0)" "$state"

echo "== 2. 全画面アプリの Space に切り替えても、ふきだしだけが見えることはない"
"${W[@]}" watch 7 > "$WORK/watch.txt" &
WATCH_PID=$!
sleep 0.5
"$WORK/bin/nativefs" 4
wait $WATCH_PID
watch=$(cat "$WORK/watch.txt")
bubble_only=$(echo "$watch" | sed -E 's/.*bubbleOnly=([0-9]+).*/\1/')
none=$(echo "$watch" | sed -E 's/.*none=([0-9]+).*/\1/')
check "ふきだしだけが出ている瞬間がない" "$([ "$bubble_only" = 0 ] && echo 1 || echo 0)" "$watch"
check "全画面の Space では、どちらも出ていない" "$([ "$none" -gt 0 ] && echo 1 || echo 0)" "none=$none"

echo "== 3. デスクトップに戻ると、ふきだしもまた出る"
sleep 2
state=$("${W[@]}" list)
check "マスコットとふきだしが出ている" "$([ "$state" = "mascot=1 bubble=1" ] && echo 1 || echo 0)" "$state"

echo "== 4. プレゼンテーション（メニューバーも隠して画面を覆う）の間は、ふきだしが消える"
"$WORK/bin/present" 5 &
PRESENT_PID=$!
sleep 3
state=$("${W[@]}" list)
wait $PRESENT_PID
check "ふきだしが出ていない" "$(echo "$state" | grep -q "bubble=0" && echo 1 || echo 0)" "$state"

echo
if [ $failures -eq 0 ]; then
  echo "すべて通りました"
else
  echo "$failures 件失敗しました（アプリのログ: $WORK/app.log）"
fi
exit $failures
