use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow,
};

const MAIN_WINDOW: &str = "main";
const CURSOR_POLL_INTERVAL: Duration = Duration::from_millis(30);
const WANDER_TICK: Duration = Duration::from_millis(16);
/// 歩く速さ（論理ピクセル/秒）
const WALK_SPEED: f64 = 60.0;
/// 目的地に着いてから次に歩き出すまでの待ち時間（ミリ秒）
const IDLE_MIN_MS: u64 = 2000;
const IDLE_MAX_MS: u64 = 6000;

/// キャラクターにカーソルが乗っている間（＝クリック・ドラッグ可能な間）は移動を止める
#[derive(Default)]
struct Hovering(Arc<AtomicBool>);

/// フロントエンドへ送る移動状態。dir は進行方向（-1: 左, 1: 右）
#[derive(Clone, Copy, Serialize)]
struct MovePayload {
    moving: bool,
    dir: i8,
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
    hovering: State<Hovering>,
    ignore: bool,
) -> Result<(), String> {
    hovering.0.store(!ignore, Ordering::Relaxed);
    window
        .set_ignore_cursor_events(ignore)
        .map_err(|e| e.to_string())
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

/// 依存クレートを増やさないための簡易乱数（xorshift64）
struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545_f491_4f6c_dd1d);
        Self(seed | 1)
    }

    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.next_f64()
    }
}

/// 現在のモニターの作業領域（メニューバーと Dock を除く）内で、ランダムな目的地を選ぶ
fn pick_target(window: &WebviewWindow, rng: &mut Rng) -> Option<PhysicalPosition<f64>> {
    let monitor = window.current_monitor().ok()??;
    let area = monitor.work_area();
    let size = window.outer_size().ok()?;
    let max_x = (area.size.width as f64 - size.width as f64).max(0.0);
    let max_y = (area.size.height as f64 - size.height as f64).max(0.0);
    Some(PhysicalPosition::new(
        area.position.x as f64 + rng.range(0.0, max_x),
        area.position.y as f64 + rng.range(0.0, max_y),
    ))
}

/// 目的地へ歩く → しばらく休む、を繰り返してデスクトップ上をうろうろさせる
fn spawn_wanderer(app: AppHandle, hovering: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut rng = Rng::new();
        let mut target: Option<PhysicalPosition<f64>> = None;
        let mut current: Option<PhysicalPosition<f64>> = None;
        let mut idle_until = Instant::now() + Duration::from_millis(IDLE_MIN_MS);
        let mut moving = false;

        let set_moving = |window: &WebviewWindow, moving: &mut bool, value: bool, dir: i8| {
            if *moving != value {
                *moving = value;
                let _ = window.emit("mascot-move", MovePayload { moving: value, dir });
            }
        };

        loop {
            thread::sleep(WANDER_TICK);
            let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
                continue;
            };

            // ホバー中・非表示中は一時停止。ドラッグで動かされた可能性があるので位置は読み直す
            if hovering.load(Ordering::Relaxed) || !window.is_visible().unwrap_or(false) {
                set_moving(&window, &mut moving, false, 0);
                current = None;
                target = None;
                idle_until = Instant::now() + Duration::from_millis(IDLE_MIN_MS);
                continue;
            }

            let Some(goal) = target else {
                if Instant::now() >= idle_until {
                    target = pick_target(&window, &mut rng);
                }
                continue;
            };

            let pos = match current {
                Some(p) => p,
                None => match window.outer_position() {
                    Ok(p) => PhysicalPosition::new(p.x as f64, p.y as f64),
                    Err(_) => continue,
                },
            };

            let scale = window.scale_factor().unwrap_or(1.0);
            let step = WALK_SPEED * scale * WANDER_TICK.as_secs_f64();
            let (dx, dy) = (goal.x - pos.x, goal.y - pos.y);
            let dist = dx.hypot(dy);

            if dist <= step {
                let _ = window.set_position(PhysicalPosition::new(
                    goal.x.round() as i32,
                    goal.y.round() as i32,
                ));
                set_moving(&window, &mut moving, false, 0);
                current = None;
                target = None;
                let idle = rng.range(IDLE_MIN_MS as f64, IDLE_MAX_MS as f64) as u64;
                idle_until = Instant::now() + Duration::from_millis(idle);
                continue;
            }

            set_moving(&window, &mut moving, true, if dx < 0.0 { -1 } else { 1 });
            let next = PhysicalPosition::new(pos.x + dx / dist * step, pos.y + dy / dist * step);
            let _ = window.set_position(PhysicalPosition::new(
                next.x.round() as i32,
                next.y.round() as i32,
            ));
            current = Some(next);
        }
    });
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "表示 / 非表示", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &PredefinedMenuItem::separator(app)?, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Desktop Mascot")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => {
                if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Hovering::default())
        .invoke_handler(tauri::generate_handler![set_click_through])
        .setup(|app| {
            // Dock に表示せず、メニューバー常駐アプリとして動かす
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            setup_tray(app)?;

            let window = app.get_webview_window(MAIN_WINDOW).unwrap();
            // 初期状態は全体をクリックスルーにする
            window.set_ignore_cursor_events(true)?;
            // 全てのスペース（デスクトップ）で表示する
            window.set_visible_on_all_workspaces(true)?;

            spawn_cursor_watcher(app.handle().clone());
            spawn_wanderer(app.handle().clone(), app.state::<Hovering>().0.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
