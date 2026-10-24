# Desktop Mascot のビルド用コマンド
#   make dev   開発モードで起動（ホットリロード）
#   make app   リリースビルドの .app を作成
#   make dmg   配布用の .dmg を作成（.app も残る）
#   make icon  scripts/gen-icon.mjs からアプリアイコンを作り直す
#   make test  テストを実行（フロントエンド + Rust）
#   make check CI と同じ確認（型チェック・テスト・fmt・clippy）
#   make bump  バージョンを上げる（LEVEL=minor / major、または VERSION=1.2.3）
#   make notes リリースノートの下書きを出す（TAG=v1.2.3、省略時は今のバージョン）

BUNDLE_DIR := src-tauri/target/release/bundle
ICON_SRC := src-tauri/icons/source.svg
TRAY_ICON := src-tauri/icons/tray-icon.png
ICON_TMP := .icon-build
# macOS 版で使うアイコンだけを src-tauri/icons にコピーする
ICON_FILES := 32x32.png 128x128.png 128x128@2x.png icon.icns icon.ico icon.png

.PHONY: help dev app dmg icon test check bump notes clean

help:
	@echo "make dev   開発モードで起動"
	@echo "make app   .app を作成 ($(BUNDLE_DIR)/macos)"
	@echo "make dmg   .dmg を作成 ($(BUNDLE_DIR)/dmg)"
	@echo "make icon  アプリアイコンを作り直す"
	@echo "make test  テストを実行"
	@echo "make check CI と同じ確認を実行"
	@echo "make bump  バージョンを上げる（LEVEL=minor / major、または VERSION=1.2.3）"
	@echo "make notes リリースノートの下書きを出す（TAG=v1.2.3）"
	@echo "make clean ビルド成果物を削除"

node_modules: package.json package-lock.json
	npm install
	@touch node_modules

dev: node_modules
	npm run tauri dev

app: node_modules
	npm run tauri build -- --bundles app
	@echo "作成しました: $(BUNDLE_DIR)/macos/"

# dmg だけを指定すると Tauri が途中で .app を消すため、両方を作って残す
dmg: node_modules
	npm run tauri build -- --bundles app,dmg
	@echo "作成しました: $(BUNDLE_DIR)/dmg/"

icon: node_modules
	node scripts/gen-icon.mjs $(ICON_SRC) $(TRAY_ICON)
	rm -rf $(ICON_TMP)
	npx tauri icon $(ICON_SRC) -o $(ICON_TMP)
	cd $(ICON_TMP) && cp $(ICON_FILES) ../src-tauri/icons/
	rm -rf $(ICON_TMP)

test: node_modules
	npm test
	cd src-tauri && cargo test

check: node_modules
	npm run build
	npm test
	cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test

# 例: make bump / make bump LEVEL=minor / make bump VERSION=1.0.0
bump:
	node scripts/bump.mjs $(or $(VERSION),$(LEVEL),patch)

# 例: make notes TAG=v0.8.0（タグがまだなければ、HEAD までの下書きにする）
notes:
	@tag=$${TAG:-v$$(node -p "require('./src-tauri/tauri.conf.json').version")}; \
	if git rev-parse -q --verify "refs/tags/$$tag" > /dev/null; then \
		node scripts/release-notes.mjs "$$tag"; \
	else \
		node scripts/release-notes.mjs HEAD "$$(git describe --tags --abbrev=0)" | sed "s/HEAD/$$tag/g; s/Desktop-Mascot_HEAD_/Desktop-Mascot_$${tag#v}_/"; \
	fi

clean:
	rm -rf dist $(BUNDLE_DIR)
