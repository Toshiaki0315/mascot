//! リマインダーなどの大きなふきだし：マスコットとは別の、透明なウィンドウに出す。
//! マスコットのウィンドウ（148×206）には収まらない大きさにできるよう、文章に合わせて大きさを変え、
//! マスコットの頭の上に置いて、マスコットが動いたら追いかける

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::{
    geometry::{self, Rect},
    Shared, BASE_FOOT_OFFSET, BASE_HEAD_HEIGHT, BASE_WIDTH, MAIN_WINDOW,
};

pub const LABEL: &str = "bubble";
/// マスコットの頭とふきだしのしっぽの間（等倍のとき）
const GAP: f64 = 4.0;
/// 追いかける間隔
const FOLLOW_INTERVAL: Duration = Duration::from_millis(33);

/// いま出しているふきだしの状態
pub struct BubbleState {
    visible: AtomicBool,
    size: Mutex<(f64, f64)>,
    /// いま出す（出している）のが、ふつうのおしゃべりか（クリックを通す）
    chat: AtomicBool,
}

impl BubbleState {
    pub fn new() -> Self {
        Self {
            visible: AtomicBool::new(false),
            size: Mutex::new((0.0, 0.0)),
            chat: AtomicBool::new(false),
        }
    }
}

/// ふきだしのページへ送る中身
#[derive(Clone, Serialize)]
struct Content {
    text: String,
    /// 「5 分後」のボタンを付けるか
    snooze: bool,
    /// マスコットの大きさの倍率（文字の大きさもこれに合わせる）
    scale: f64,
    /// ふつうのおしゃべりか（ボタンはなく、クリックを通す）
    chat: bool,
}

/// ふきだしのウィンドウを、見えない状態で作っておく（出すときの待ち時間をなくすため）
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("bubble.html".into()))
        .title("Mascot bubble")
        .inner_size(10.0, 10.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .accept_first_mouse(true)
        .visible(false)
        .build()?;
    window.set_visible_on_all_workspaces(true)?;
    Ok(())
}

/// ふきだしの位置。マスコットの頭の真上にしっぽが来るように中央揃えで置き、
/// 画面の作業領域（メニューバーと Dock を除く）からはみ出さないようにする
pub fn bubble_position(mascot: Rect, size: (f64, f64), work: Rect) -> (f64, f64) {
    let scale = mascot.w / BASE_WIDTH;
    let head_top = mascot.y + mascot.h - (BASE_FOOT_OFFSET + BASE_HEAD_HEIGHT) * scale;
    let (w, h) = size;
    let x = (mascot.x + mascot.w / 2.0 - w / 2.0).clamp(work.x, (work.x + work.w - w).max(work.x));
    // 上にはみ出すときは、画面の上端に合わせる（マスコットの頭にかかる）
    let y = (head_top - GAP * scale - h).max(work.y);
    (x, y)
}

fn work_area_of(window: &WebviewWindow, mascot: Rect) -> Option<Rect> {
    let screens = geometry::screens(window);
    screens
        .iter()
        .find(|s| s.full.contains(mascot.center()))
        .or_else(|| screens.first())
        .map(|s| s.work)
}

fn place(main: &WebviewWindow, bubble: &WebviewWindow, size: (f64, f64)) {
    let Some(mascot) = geometry::window_rect(main) else {
        return;
    };
    let Some(work) = work_area_of(main, mascot) else {
        return;
    };
    let (x, y) = bubble_position(mascot, size, work);
    let _ = bubble.set_position(LogicalPosition::new(x, y));
}

/// マスコットのメインのウィンドウから呼ばれ、ふきだしの中身を送る。
/// 実際に出すのは、ページが大きさを測って `bubble_resize` を呼んでから
#[tauri::command]
pub fn show_bubble(
    app: AppHandle,
    state: State<Arc<BubbleState>>,
    text: String,
    snooze: bool,
    chat: bool,
) {
    let (Some(main), Some(bubble)) = (
        app.get_webview_window(MAIN_WINDOW),
        app.get_webview_window(LABEL),
    ) else {
        return;
    };
    let scale = geometry::window_rect(&main).map_or(1.0, |r| r.w / BASE_WIDTH);
    state.chat.store(chat, Ordering::Relaxed);
    let _ = bubble.emit(
        "bubble-content",
        Content {
            text,
            snooze,
            scale,
            chat,
        },
    );
}

/// ふきだしのページが、文章に合わせた大きさを測って知らせてくる
#[tauri::command]
pub fn bubble_resize(app: AppHandle, state: State<Arc<BubbleState>>, w: f64, h: f64) {
    let (Some(main), Some(bubble)) = (
        app.get_webview_window(MAIN_WINDOW),
        app.get_webview_window(LABEL),
    ) else {
        return;
    };
    *state.size.lock().unwrap() = (w, h);
    let _ = bubble.set_size(LogicalSize::new(w, h));
    place(&main, &bubble, (w, h));
    // ふつうのおしゃべりは、後ろのアプリのクリックをじゃましない
    let _ = bubble.set_ignore_cursor_events(state.chat.load(Ordering::Relaxed));
    let _ = bubble.show();
    state.visible.store(true, Ordering::Relaxed);
}

/// ふきだしを消す（次のふきだしを出す前にも呼ぶ）
#[tauri::command]
pub fn hide_bubble(app: AppHandle, state: State<Arc<BubbleState>>) {
    hide_now(&app, &state);
}

fn hide_now(app: &AppHandle, state: &BubbleState) {
    state.visible.store(false, Ordering::Relaxed);
    if let Some(bubble) = app.get_webview_window(LABEL) {
        let _ = bubble.hide();
    }
}

/// マスコットを隠したときなど、Rust 側の都合でふきだしを消す。
/// マスコットのほうの「いま出している」状態も戻すため、閉じたことを知らせる
pub fn dismiss(app: &AppHandle) {
    if let Some(state) = app.try_state::<Arc<BubbleState>>() {
        if state.visible.load(Ordering::Relaxed) {
            hide_now(app, &state);
            if let Some(main) = app.get_webview_window(MAIN_WINDOW) {
                let _ = main.emit("reminder-action", "close");
            }
        }
    }
}

/// ふきだしのボタン（5 分後 / 閉じる）や、ふきだしのクリックを、マスコット側へ伝える
#[tauri::command]
pub fn bubble_action(app: AppHandle, action: String) {
    if let Some(main) = app.get_webview_window(MAIN_WINDOW) {
        let _ = main.emit("reminder-action", action);
    }
}

/// マスコットが動いたら、ふきだしも追いかける
pub fn spawn_follower(app: AppHandle, shared: Arc<Shared>, state: Arc<BubbleState>) {
    thread::spawn(move || {
        let mut last: Option<(f64, f64)> = None;
        loop {
            thread::sleep(FOLLOW_INTERVAL);
            if !state.visible.load(Ordering::Relaxed) {
                last = None;
                continue;
            }
            if !shared.visible.load(Ordering::Relaxed) {
                dismiss(&app);
                continue;
            }
            let (Some(main), Some(bubble)) = (
                app.get_webview_window(MAIN_WINDOW),
                app.get_webview_window(LABEL),
            ) else {
                continue;
            };
            let size = *state.size.lock().unwrap();
            let Some(mascot) = geometry::window_rect(&main) else {
                continue;
            };
            let Some(work) = work_area_of(&main, mascot) else {
                continue;
            };
            let pos = bubble_position(mascot, size, work);
            if last != Some(pos) {
                last = Some(pos);
                let _ = bubble.set_position(LogicalPosition::new(pos.0, pos.1));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect {
        x: 0.0,
        y: 30.0,
        w: 1800.0,
        h: 1100.0,
    };

    #[test]
    fn sits_above_the_head_centered_on_the_mascot() {
        let mascot = Rect::new(900.0, 700.0, 148.0, 206.0);
        let (x, y) = bubble_position(mascot, (200.0, 100.0), WORK);
        assert_eq!(x, 900.0 + 74.0 - 100.0);
        // 頭の上端 = 700 + 206 - (21 + 105) = 780。しっぽの分の間を空けて、ふきだしの下端がその少し上
        assert_eq!(y, 780.0 - 4.0 - 100.0);
    }

    #[test]
    fn scales_with_the_mascot() {
        let mascot = Rect::new(900.0, 700.0, 296.0, 412.0);
        let (_, y) = bubble_position(mascot, (200.0, 100.0), WORK);
        // 2 倍のとき、頭の上端は 700 + 412 - 252 = 860、間は 8
        assert_eq!(y, 860.0 - 8.0 - 100.0);
    }

    #[test]
    fn stays_inside_the_work_area_horizontally() {
        let near_left = Rect::new(-10.0, 700.0, 148.0, 206.0);
        assert_eq!(bubble_position(near_left, (300.0, 80.0), WORK).0, 0.0);
        let near_right = Rect::new(1700.0, 700.0, 148.0, 206.0);
        assert_eq!(bubble_position(near_right, (300.0, 80.0), WORK).0, 1500.0);
    }

    #[test]
    fn overlaps_the_head_when_there_is_no_room_above() {
        let high = Rect::new(900.0, 40.0, 148.0, 206.0);
        let (_, y) = bubble_position(high, (200.0, 300.0), WORK);
        assert_eq!(y, WORK.y);
    }
}
