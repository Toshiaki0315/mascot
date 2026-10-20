# Desktop Mascot App

Tauri v2 (Rust + TypeScript) を使用した macOS 向けデスクトップマスコットアプリです。
ドット絵のネコがデスクトップ上を歩き回ります。

## 開発環境の構築
1. **前提条件**:
   * Node.js (v20.19 以降、または v22.12 以降。Vite 8 の要件)
   * Rust / Cargo
   * macOS 開発環境 (Xcode Command Line Tools)
2. **依存関係のインストール**:
   プロジェクトルートで以下のコマンドを実行します。
   ```bash
   npm install
   ```

## コマンド
よく使う操作は `make` で実行できます（`npm install` が済んでいなければ自動で実行されます）。

| コマンド | 内容 | 出力先 |
| --- | --- | --- |
| `make dev` | 開発モードで起動（ホットリロード） | — |
| `make app` | リリースビルドの `.app` を作成 | `src-tauri/target/release/bundle/macos/` |
| `make dmg` | 配布用の `.dmg` を作成（`.app` も残る） | `src-tauri/target/release/bundle/dmg/` |
| `make icon` | `scripts/gen-icon.mjs` からアプリアイコンを作り直す | `src-tauri/icons/` |
| `make clean` | ビルド成果物を削除 | — |

※ 初回はRustのコンパイルが行われるため、数分程度時間がかかります。
※ 作成した `.app` / `.dmg` はコード署名していないため、他の Mac で開くと Gatekeeper の警告が出ます。右クリック →「開く」で起動できます。

## アプリの使い方
*   アプリを起動すると、デスクトップ上にドット絵のマスコットが表示されます。Dock には表示されません。
*   マスコットは常に最前面に表示されますが、キャラクター以外の透明な部分はクリックを透過するため、背後のアプリの操作を邪魔しません。キーボードフォーカスも奪いません。
*   マスコットは画面内（メニューバーと Dock を除く範囲）をランダムに歩き回り、目的地に着くと数秒休みます。
*   キャラクターにマウスを重ねると操作可能になり、その間は立ち止まります。

| 操作 | 動作 |
| --- | --- |
| クリック | ぴょんぴょん跳ねて喜ぶ（^ ^ の顔とハート） |
| ダブルクリック | くるっと 1 回転する |
| ドラッグ | 好きな場所へ移動する |

*   メニューバー（画面右上）のトレイアイコンから、マスコットの「表示 / 非表示」と「終了」ができます。

## カスタマイズ
*   **見た目**: [src/sprite.ts](src/sprite.ts) に 16×16 のドット絵を文字列で定義しています。文字を書き換えるとキャラクターが変わります。色は [src/styles.css](src/styles.css) の `.px-*` で指定しています。
*   **アプリアイコン**: [scripts/gen-icon.mjs](scripts/gen-icon.mjs) のドット絵と色を書き換えて `make icon` を実行すると、全サイズのアイコンが作り直されます。
*   **歩く速さ・休む時間**: [src-tauri/src/lib.rs](src-tauri/src/lib.rs) 先頭の `WALK_SPEED`（論理ピクセル/秒）、`IDLE_MIN_MS` / `IDLE_MAX_MS`（ミリ秒）で調整できます。

## 仕組み
*   **クリックスルー**: ウィンドウは通常 `set_ignore_cursor_events(true)` で全体がクリックを透過します。透過中は WebView にマウスイベントが届かないため、Rust 側がカーソル位置をポーリングしてフロントエンドへ送り、TypeScript 側でキャラクターのドットに重なっているかを判定して、Rust の `set_click_through` コマンドで透過の ON/OFF を切り替えます。
*   **ランダム移動**: Rust 側のスレッドがウィンドウの位置を少しずつ動かし、進行方向をフロントエンドへ通知して、歩くアニメーションと向きを切り替えます。

## プロジェクト構成
```
├── Makefile                # make dev / app / dmg / icon
├── index.html              # キャラクターを描く SVG の入れ物
├── scripts/gen-icon.mjs    # アプリアイコン（ドット絵）の生成スクリプト
├── src/
│   ├── main.ts             # ホバー判定、クリック・ダブルクリック・ドラッグの処理
│   ├── sprite.ts           # ドット絵の定義と SVG への描画
│   └── styles.css          # 色とアニメーション
└── src-tauri/
    ├── tauri.conf.json     # ウィンドウ設定（透過・最前面・フォーカスなし、148×158）
    ├── capabilities/       # フロントエンドに許可する API
    └── src/lib.rs          # トレイ、クリックスルー、カーソル監視、ランダム移動
```

