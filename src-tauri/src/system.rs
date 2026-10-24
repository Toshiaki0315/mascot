//! Tauri からは取れない macOS の情報を CoreGraphics から読む。
//! どの API も画面収録やアクセシビリティなどの権限なしで使える。

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::CFString,
};

use crate::geometry::Rect;

#[repr(C)]
#[derive(Default)]
struct CGRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

const K_CG_WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1 << 0;
const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
const K_CG_NULL_WINDOW_ID: u32 = 0;
const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION: i32 = 0;
const K_CG_ANY_INPUT_EVENT_TYPE: u32 = !0;
const K_CG_MOUSE_BUTTON_LEFT: u32 = 0;
/// 通常のアプリのウィンドウ
const NORMAL_LAYER: i64 = 0;
/// メニューバー（kCGMainMenuWindowLevel）
const MENU_BAR_LAYER: i64 = 24;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> bool;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayIsAsleep(display: u32) -> i32;
    fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

/// キーボード・マウスなど、最後に何か入力されてからの秒数
pub fn seconds_since_last_input() -> f64 {
    unsafe {
        CGEventSourceSecondsSinceLastEventType(
            K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION,
            K_CG_ANY_INPUT_EVENT_TYPE,
        )
    }
}

/// メイン画面のディスプレイが、スリープ（消灯）しているか
pub fn is_display_asleep() -> bool {
    unsafe { CGDisplayIsAsleep(CGMainDisplayID()) != 0 }
}

/// ログインセッションの情報（画面ロック中か、いまこのセッションが前面か）の、真偽の項目を読む
fn session_flag(key: &str) -> Option<bool> {
    let raw = unsafe { CGSessionCopyCurrentDictionary() };
    if raw.is_null() {
        return None;
    }
    let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_create_rule(raw) };
    dict.find(CFString::new(key))
        .and_then(|v| v.downcast::<CFBoolean>())
        .map(bool::from)
}

/// 画面ロックの最中か
pub fn is_screen_locked() -> bool {
    session_flag("CGSSessionScreenIsLocked").unwrap_or(false)
}

/// いま、このセッションが前面（コンソール）にいないか（ユーザーの切り替えで、別のユーザーが使っている間）
pub fn is_session_in_background() -> bool {
    session_flag("kCGSSessionOnConsoleKey") == Some(false)
}

/// 画面を誰も見ていない状態か：ディスプレイが消えている、画面がロックされている、
/// 別のユーザーに切り替わっている。この間は、マスコットの動きを止めてよい
pub fn is_screen_inactive() -> bool {
    inactive(
        is_display_asleep(),
        is_screen_locked(),
        is_session_in_background(),
    )
}

fn inactive(display_asleep: bool, locked: bool, in_background: bool) -> bool {
    display_asleep || locked || in_background
}

/// マウスの左ボタンが押されているか（ドラッグの終わりを知るために使う）
pub fn is_left_button_down() -> bool {
    unsafe {
        CGEventSourceButtonState(
            K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION,
            K_CG_MOUSE_BUTTON_LEFT,
        )
    }
}

/// 画面に出ているウィンドウ 1 つ分の情報
#[derive(Clone, Copy, Debug)]
pub struct WindowInfo {
    pub id: i64,
    pub layer: i64,
    pub bounds: Rect,
    /// このアプリ自身のウィンドウか
    pub own: bool,
}

/// 画面に出ているウィンドウを手前から順に返す
pub fn on_screen_windows() -> Vec<WindowInfo> {
    let own_pid = std::process::id() as i64;
    let id_key = CFString::from_static_string("kCGWindowNumber");
    let layer_key = CFString::from_static_string("kCGWindowLayer");
    let pid_key = CFString::from_static_string("kCGWindowOwnerPID");
    let bounds_key = CFString::from_static_string("kCGWindowBounds");

    let windows: CFArray<CFDictionary<CFString, CFType>> = unsafe {
        let raw = CGWindowListCopyWindowInfo(
            K_CG_WINDOW_LIST_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
            K_CG_NULL_WINDOW_ID,
        );
        if raw.is_null() {
            return Vec::new();
        }
        CFArray::wrap_under_create_rule(raw)
    };

    let number = |dict: &CFDictionary<CFString, CFType>, key: &CFString| {
        dict.find(key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
    };

    windows
        .iter()
        .filter_map(|dict| {
            let bounds = dict
                .find(&bounds_key)
                .and_then(|v| v.downcast::<CFDictionary>())?;
            let mut r = CGRect::default();
            if !unsafe {
                CGRectMakeWithDictionaryRepresentation(bounds.as_concrete_TypeRef(), &mut r)
            } {
                return None;
            }
            Some(WindowInfo {
                id: number(&dict, &id_key)?,
                layer: number(&dict, &layer_key)?,
                bounds: Rect::new(r.x, r.y, r.width, r.height),
                own: number(&dict, &pid_key) == Some(own_pid),
            })
        })
        .collect()
}

/// ほかのアプリの通常のウィンドウ（手前から順）
pub fn app_windows(windows: &[WindowInfo]) -> impl Iterator<Item = &WindowInfo> {
    windows.iter().filter(|w| w.layer == NORMAL_LAYER && !w.own)
}

/// 指定した画面が全画面表示やプレゼンテーション中かを判定する。
/// ほかのアプリの一番手前のウィンドウが画面全体を覆っていて、
/// しかもメニューバーが消えているときだけ「全画面」とみなす
/// （メニューバーが見えているなら、ふつうのデスクトップ作業中なので隠れない）
pub fn is_fullscreen(windows: &[WindowInfo], screen: Rect) -> bool {
    let menu_bar_visible = windows
        .iter()
        .any(|w| w.layer == MENU_BAR_LAYER && w.bounds.intersects(&screen) && w.bounds.h > 0.0);
    if menu_bar_visible {
        return false;
    }
    app_windows(windows)
        .find(|w| w.bounds.intersects(&screen))
        .is_some_and(|w| w.bounds.covers(&screen))
}

/// 点 (x, y) を、指定した位置より手前にあるウィンドウが隠しているか
pub fn is_covered_in_front(windows: &[WindowInfo], index: usize, point: (f64, f64)) -> bool {
    windows[..index]
        .iter()
        .any(|w| w.layer == NORMAL_LAYER && !w.own && w.bounds.contains(point))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: i64, layer: i64, bounds: Rect) -> WindowInfo {
        WindowInfo {
            id,
            layer,
            bounds,
            own: false,
        }
    }

    const SCREEN: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1800.0,
        h: 1169.0,
    };

    #[test]
    fn fullscreen_requires_hidden_menu_bar() {
        let menu_bar = win(1, MENU_BAR_LAYER, Rect::new(0.0, 0.0, 1800.0, 39.0));
        let cover = win(2, NORMAL_LAYER, SCREEN);
        assert!(!is_fullscreen(&[menu_bar, cover], SCREEN));
        assert!(is_fullscreen(&[cover], SCREEN));
    }

    #[test]
    fn maximized_window_is_not_fullscreen() {
        let maximized = win(2, NORMAL_LAYER, Rect::new(0.0, 39.0, 1800.0, 1130.0));
        assert!(!is_fullscreen(&[maximized], SCREEN));
    }

    #[test]
    fn only_frontmost_window_on_the_screen_counts() {
        let small = win(2, NORMAL_LAYER, Rect::new(100.0, 100.0, 400.0, 300.0));
        let cover = win(3, NORMAL_LAYER, SCREEN);
        assert!(!is_fullscreen(&[small, cover], SCREEN));
    }

    #[test]
    fn own_windows_are_ignored() {
        let mut mine = win(2, NORMAL_LAYER, SCREEN);
        mine.own = true;
        assert!(!is_fullscreen(&[mine], SCREEN));
    }

    #[test]
    fn covered_in_front() {
        let front = win(1, NORMAL_LAYER, Rect::new(0.0, 0.0, 100.0, 100.0));
        let back = win(2, NORMAL_LAYER, Rect::new(0.0, 0.0, 500.0, 500.0));
        let list = [front, back];
        assert!(is_covered_in_front(&list, 1, (50.0, 50.0)));
        assert!(!is_covered_in_front(&list, 1, (300.0, 300.0)));
        assert!(!is_covered_in_front(&list, 0, (50.0, 50.0)));
    }

    #[test]
    fn inactive_when_any_reason_applies() {
        assert!(!inactive(false, false, false));
        assert!(inactive(true, false, false));
        assert!(inactive(false, true, false));
        assert!(inactive(false, false, true));
    }

    #[test]
    fn the_real_session_calls_do_not_crash() {
        // 実機の状態はわからないが、呼べること（クラッシュしないこと）は確かめる
        eprintln!(
            "display asleep={} locked={} in background={}",
            is_display_asleep(),
            is_screen_locked(),
            is_session_in_background()
        );
        let _ = is_screen_inactive();
    }
}
