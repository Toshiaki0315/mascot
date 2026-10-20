mod geometry;
mod settings;
mod system;
mod tray;
mod wander;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_autostart::MacosLauncher;

use settings::Store;

pub(crate) const MAIN_WINDOW: &str = "main";
const CURSOR_POLL_INTERVAL: Duration = Duration::from_millis(30);
/// 等倍のときのウィンドウサイズ（tauri.conf.json と同じ値）
const BASE_WIDTH: f64 = 148.0;
const BASE_HEIGHT: f64 = 158.0;

/// スレッド間で共有する状態
pub(crate) struct Shared {
    /// キャラクターにカーソルが乗っている間（＝クリック・ドラッグ可能な間）は移動を止める
    hovering: AtomicBool,
    /// 大きさの変更などでウィンドウが動いたので、歩き方を計算し直してほしい
    reset: AtomicBool,
    /// フロントエンドが（再）読み込みされたので、今の状態を送り直してほしい
    resync: AtomicBool,
    store: Store,
}

/// フロントエンドへ送るカーソル座標（ウィンドウ左上基準の論理ピクセル）
#[derive(Clone, Copy, PartialEq, Serialize)]
struct CursorPayload {
    x: f64,
    y: f64,
}

/// TypeScript 側から呼ばれ、クリックスルーの ON/OFF を切り替える
#[tauri::command]
fn set_click_through(
    window: WebviewWindow,
    shared: State<Arc<Shared>>,
    ignore: bool,
) -> Result<(), String> {
    shared.hovering.store(!ignore, Ordering::Relaxed);
    window
        .set_ignore_cursor_events(ignore)
        .map_err(|e| e.to_string())
}

/// フロントエンドがイベントを受け取れるようになったら呼ばれる。
/// それより前に送った通知は届いていないので、状態を送り直す
#[tauri::command]
fn frontend_ready(shared: State<Arc<Shared>>) {
    shared.resync.store(true, Ordering::Relaxed);
}

/// クリックスルー中は WebView にマウスイベントが届かないため、
/// グローバルなカーソル位置をポーリングしてフロントエンドへ通知する
fn spawn_cursor_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut last: Option<CursorPayload> = None;
        loop {
            thread::sleep(CURSOR_POLL_INTERVAL);
            let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
                continue;
            };
            if !window.is_visible().unwrap_or(false) {
                continue;
            }
            let (Ok(cursor), Ok(origin), Ok(scale)) = (
                window.cursor_position(),
                window.inner_position(),
                window.scale_factor(),
            ) else {
                continue;
            };
            let payload = CursorPayload {
                x: (cursor.x - origin.x as f64) / scale,
                y: (cursor.y - origin.y as f64) / scale,
            };
            if last != Some(payload) {
                last = Some(payload);
                let _ = window.emit("cursor-moved", payload);
            }
        }
    });
}

pub(crate) fn save_current_position(window: &WebviewWindow, shared: &Shared) {
    if let Some(rect) = geometry::window_rect(window) {
        shared.store.update(|s| s.position = Some((rect.x, rect.y)));
    }
}

/// 大きさを変える。足元（下端の中央）の位置はそのままにする
pub(crate) fn apply_scale(window: &WebviewWindow, scale: f64) {
    let (w, h) = (BASE_WIDTH * scale, BASE_HEIGHT * scale);
    if let Some(rect) = geometry::window_rect(window) {
        let (cx, bottom) = (rect.x + rect.w / 2.0, rect.y + rect.h);
        geometry::resize_window(window, w, h);
        geometry::move_window(window, cx - w / 2.0, bottom - h);
    } else {
        geometry::resize_window(window, w, h);
    }
    let shared = window.state::<Arc<Shared>>();
    shared.reset.store(true, Ordering::Relaxed);
}

/// 前回終了した時の大きさ・位置・表示状態に戻す
fn restore(window: &WebviewWindow, shared: &Shared) {
    let settings = shared.store.get();
    let (w, h) = (BASE_WIDTH * settings.scale, BASE_HEIGHT * settings.scale);
    geometry::resize_window(window, w, h);
    if let Some((x, y)) = settings.position {
        // 前回の位置がもう存在しない画面なら使わない（bring_back_if_lost が戻す）
        if geometry::is_on_screen(window, geometry::Rect::new(x, y, w, h)) {
            geometry::move_window(window, x, y);
        }
    }
    geometry::bring_back_if_lost(window);
    if !settings.visible {
        let _ = window.hide();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![set_click_through, frontend_ready])
        .setup(|app| {
            // Dock に表示せず、メニューバー常駐アプリとして動かす
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let path = app.path().app_config_dir()?.join("settings.json");
            let shared = Arc::new(Shared {
                hovering: AtomicBool::new(false),
                reset: AtomicBool::new(false),
                resync: AtomicBool::new(false),
                store: Store::load(path),
            });
            app.manage(shared.clone());

            tray::setup(app, &shared)?;

            let window = app.get_webview_window(MAIN_WINDOW).unwrap();
            // 初期状態は全体をクリックスルーにする
            window.set_ignore_cursor_events(true)?;
            // 全てのスペース（デスクトップ）で表示する
            window.set_visible_on_all_workspaces(true)?;
            restore(&window, &shared);

            spawn_cursor_watcher(app.handle().clone());
            wander::spawn(app.handle().clone(), shared);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
