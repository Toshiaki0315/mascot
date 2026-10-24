# Desktop Mascot App Specification (for Codex)

## 概要
macOS上で動作するデスクトップマスコットアプリ（Tauri: Rust + TypeScript）の開発仕様書です。

## 基本要件
*   **フレームワーク**: Tauri (Rust + TypeScript/React or Vue)
*   **ターゲットOS**: macOS

## ウィンドウ仕様
*   **サイズ**: 256x256ピクセル程度の小さなウィンドウ（キャラクターのサイズに合わせて調整）
*   **透明化**: 背景を完全に透過 (`transparent: true`, `decorations: false`)
*   **最前面表示**: 常に最前面に表示 (`alwaysOnTop: true`)
*   **フォーカス**: 起動時やクリック時にキーボードフォーカスを奪わない（通常の作業を妨げない設定）
*   **常駐化**: Dockには表示せず、メニューバー（System Tray）にのみアイコンを表示

## マウスイベントとクリックスルー（重要）
普段のPC作業を邪魔しないための実装が必要です。
1.  **基本状態（透過）**: マスコットの透明な余白部分はマウスクリックを透過し、背後のアプリを操作できるようにする（Rust側で `set_ignore_cursor_events(true)` を設定）。
2.  **動的切り替え**: マウスカーソルがマスコットの不透明な部分（キャラクター自体）に重なった時だけ、TypeScript側からRustへイベントを発火し、`set_ignore_cursor_events(false)` に切り替える。離れたら再び `true` に戻す。

## 実装してほしい機能のステップ
1. Tauriのセットアップと `tauri.conf.json` の設定
2. Rust側でのウィンドウ属性（透過、最前面、フォーカス制御）とトレイアイコンの実装
3. Rust側での `set_ignore_cursor_events` を動的に切り替えるコマンド（IPC）の実装
4. TypeScript側でのキャラクター描画（まずはプレースホルダー画像かCSSアニメーション）と、マウスホバー検知に基づくRustコマンドの呼び出し
