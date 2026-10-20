//! Tauri からは取れない macOS の情報を CoreGraphics から読む。
//! どちらの API も画面収録などの権限なしで使える。

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, TCFType},
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

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> bool;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
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

/// 指定した画面で、ほかのアプリの最前面のウィンドウが画面全体を覆っているか
/// （全画面表示やプレゼンテーション中か）を判定する
pub fn is_screen_covered(screen: Rect) -> bool {
    let own_pid = std::process::id() as i64;
    let layer_key = CFString::from_static_string("kCGWindowLayer");
    let pid_key = CFString::from_static_string("kCGWindowOwnerPID");
    let bounds_key = CFString::from_static_string("kCGWindowBounds");

    let windows: CFArray<CFDictionary<CFString, CFType>> = unsafe {
        let raw = CGWindowListCopyWindowInfo(
            K_CG_WINDOW_LIST_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
            K_CG_NULL_WINDOW_ID,
        );
        if raw.is_null() {
            return false;
        }
        CFArray::wrap_under_create_rule(raw)
    };

    let number = |dict: &CFDictionary<CFString, CFType>, key: &CFString| {
        dict.find(key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
    };

    // 一覧は手前から順に並んでいる。通常のウィンドウ（レイヤー 0）のうち、
    // この画面に掛かっている一番手前のものだけを見る
    for dict in windows.iter() {
        if number(&dict, &layer_key) != Some(0) || number(&dict, &pid_key) == Some(own_pid) {
            continue;
        }
        let Some(bounds) = dict
            .find(&bounds_key)
            .and_then(|v| v.downcast::<CFDictionary>())
        else {
            continue;
        };
        let mut r = CGRect::default();
        if !unsafe { CGRectMakeWithDictionaryRepresentation(bounds.as_concrete_TypeRef(), &mut r) }
        {
            continue;
        }
        let rect = Rect::new(r.x, r.y, r.width, r.height);
        if !rect.intersects(&screen) {
            continue;
        }
        return rect.covers(&screen);
    }
    false
}
