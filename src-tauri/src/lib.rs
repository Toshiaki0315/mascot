mod about;
mod geometry;
mod physics;
mod settings;
mod system;
mod tray;
mod wander;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_autostart::MacosLauncher;

use geometry::Rect;
use settings::Store;

pub(crate) const MAIN_WINDOW: &str = "main";
/// カーソルがキャラクターの近くにある時の監視間隔
const CURSOR_POLL_NEAR: Duration = Duration::from_millis(30);
/// カーソルが遠い時の監視間隔。遠い間はフロントエンドへも通知しない
const CURSOR_POLL_FAR: Duration = Duration::from_millis(150);
/// ウィンドウの大きさの何倍の距離までを「近い」とみなすか（視線を追う範囲より広く取る）
const CURSOR_NEAR_RANGE: f64 = 3.0;
/// 等倍のときのウィンドウサイズ（tauri.conf.json と同じ値）。
/// 上の 48pt はふきだしを出すための余白
pub(crate) const BASE_WIDTH: f64 = 148.0;
pub(crate) const BASE_HEIGHT: f64 = 206.0;
/// 等倍のとき、ウィンドウの下端から足元までの距離（下の余白 14pt + 影 1 ドット 7pt）
pub(crate) const BASE_FOOT_OFFSET: f64 = 21.0;
/// 等倍のとき、足元から耳の先までの高さ（15 ドット × 7pt）
pub(crate) const BASE_HEAD_HEIGHT: f64 = 105.0;

/// ほかのスレッドが毎回メインスレッドへ問い合わせなくて済むよう、
/// 頭脳スレッドが知っているウィンドウの位置と倍率を置いておく
#[derive(Clone, Copy)]
pub(crate) struct Geometry {
    pub rect: Rect,
    pub scale_factor: f64,
}

/// スレッド間で共有する状態
pub(crate) struct Shared {
    /// キャラクターにカーソルが乗っている間（＝クリック・ドラッグ可能な間）は移動を止める
    hovering: AtomicBool,
    /// 大きさの変更などでウィンドウが動いたので、歩き方を計算し直してほしい
    reset: AtomicBool,
    /// フロントエンドがウィンドウのドラッグを始めた（頭脳スレッドが受け取ったら false に戻す）
    drag_started: AtomicBool,
    /// フロントエンドが（再）読み込みされたので、今の状態を送り直してほしい
    resync: AtomicBool,
    /// トレイで「表示」になっているか（毎回 is_visible() を問い合わせないためのキャッシュ）
    visible: AtomicBool,
    geometry: Mutex<Option<Geometry>>,
    store: Store,
}

impl Shared {
    pub(crate) fn geometry(&self) -> Option<Geometry> {
        *self.geometry.lock().unwrap()
    }

    pub(crate) fn set_geometry(&self, geometry: Geometry) {
        *self.geometry.lock().unwrap() = Some(geometry);
    }
}

/// フロントエンドへ送るカーソル座標（ウィンドウ左上基準の論理ピクセル）
#[derive(Clone, Copy, PartialEq, Serialize)]
struct CursorPayload {
    x: f64,
    y: f64,
}

/// フロントエンドに関係する設定
#[derive(Clone, Serialize)]
struct SettingsPayload {
    chatty: bool,
    character: &'static str,
}

pub(crate) fn emit_settings(window: &WebviewWindow, shared: &Shared) {
    let settings = shared.store.get();
    let _ = window.emit(
        "mascot-settings",
        SettingsPayload {
            chatty: settings.chatty,
            character: settings.character.name(),
        },
    );
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

/// フロントエンドがウィンドウのドラッグを始める直前に呼ばれる
#[tauri::command]
fn start_drag(shared: State<Arc<Shared>>) {
    shared.drag_started.store(true, Ordering::Relaxed);
}

/// フロントエンドがイベントを受け取れるようになったら呼ばれる。
/// それより前に送った通知は届いていないので、状態を送り直す
#[tauri::command]
fn frontend_ready(window: WebviewWindow, shared: State<Arc<Shared>>) {
    shared.resync.store(true, Ordering::Relaxed);
    emit_settings(&window, &shared);
}

/// クリックスルー中は WebView にマウスイベントが届かないため、
/// グローバルなカーソル位置をポーリングしてフロントエンドへ通知する。
/// カーソルが遠い間は間隔を延ばし、通知もしない（遠ざかった時に 1 度だけ送る）
fn spawn_cursor_watcher(app: AppHandle, shared: Arc<Shared>) {
    thread::spawn(move || {
        let mut last: Option<CursorPayload> = None;
        let mut near = false;
        loop {
            thread::sleep(if near {
                CURSOR_POLL_NEAR
            } else {
                CURSOR_POLL_FAR
            });
            if !shared.visible.load(Ordering::Relaxed) {
                continue;
            }
            let (Some(window), Some(geometry)) =
                (app.get_webview_window(MAIN_WINDOW), shared.geometry())
            else {
                continue;
            };
            let Ok(cursor) = window.cursor_position() else {
                continue;
            };
            let rect = geometry.rect;
            let payload = CursorPayload {
                x: cursor.x / geometry.scale_factor - rect.x,
                y: cursor.y / geometry.scale_factor - rect.y,
            };
            let (cx, cy) = (rect.w / 2.0, rect.h / 2.0);
            let now_near =
                (payload.x - cx).hypot(payload.y - cy) < rect.w.max(rect.h) * CURSOR_NEAR_RANGE;
            if (now_near || near) && last != Some(payload) {
                last = Some(payload);
                let _ = window.emit("cursor-moved", payload);
            }
            near = now_near;
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
        if geometry::is_on_screen(window, Rect::new(x, y, w, h)) {
            geometry::move_window(window, x, y);
        }
    }
    geometry::bring_back_if_lost(window);
    shared.visible.store(settings.visible, Ordering::Relaxed);
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
        .invoke_handler(tauri::generate_handler![
            set_click_through,
            frontend_ready,
            start_drag
        ])
        .setup(|app| {
            // Dock に表示せず、メニューバー常駐アプリとして動かす
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let path = app.path().app_config_dir()?.join("settings.json");
            let shared = Arc::new(Shared {
                hovering: AtomicBool::new(false),
                reset: AtomicBool::new(false),
                drag_started: AtomicBool::new(false),
                resync: AtomicBool::new(false),
                visible: AtomicBool::new(true),
                geometry: Mutex::new(None),
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

            spawn_cursor_watcher(app.handle().clone(), shared.clone());
            wander::spawn(app.handle().clone(), shared);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
