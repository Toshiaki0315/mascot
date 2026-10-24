//! 「一般」の設定（歩き回る・歩く速さ・大きさ・おしゃべり・全画面アプリの上では隠れる・ログイン時に起動）。
//! 設定の画面から読み書きする

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    apply_scale, emit_settings, save_current_position,
    settings::{Settings, Speed, SCALES},
    Shared, MAIN_WINDOW,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct General {
    pub walking: bool,
    pub speed: Speed,
    /// 大きさ（等倍に対する倍率。SCALES のどれか）
    pub scale: f64,
    pub chatty: bool,
    pub hide_on_fullscreen: bool,
    /// ログイン時に起動する
    pub autostart: bool,
}

impl General {
    fn of(s: &Settings, autostart: bool) -> Self {
        Self {
            walking: s.walking,
            speed: s.speed,
            scale: s.scale,
            chatty: s.chatty,
            hide_on_fullscreen: s.hide_on_fullscreen,
            autostart,
        }
    }

    fn valid(&self) -> bool {
        SCALES.contains(&self.scale)
    }
}

fn current(app: &AppHandle, shared: &Shared) -> General {
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    General::of(&shared.store.get(), autostart)
}

#[tauri::command]
pub fn get_general(app: AppHandle, shared: State<Arc<Shared>>) -> General {
    current(&app, &shared)
}

/// 設定を保存して、すぐ反映する（大きさはウィンドウの大きさを変える）。
/// ログイン時の起動は、登録に失敗することがあるので、実際の状態を返す
#[tauri::command]
pub fn set_general(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    general: General,
) -> Result<General, String> {
    if !general.valid() {
        return Err("大きさが正しくありません".into());
    }
    let before = shared.store.get();
    shared.store.update(|s| {
        s.walking = general.walking;
        s.speed = general.speed;
        s.chatty = general.chatty;
        s.hide_on_fullscreen = general.hide_on_fullscreen;
    });

    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        if before.scale != general.scale {
            apply_scale(&window, general.scale);
            shared.store.update(|s| s.scale = general.scale);
            save_current_position(&window, &shared);
        }
        if before.chatty != general.chatty {
            emit_settings(&window, &shared);
        }
    }

    let launcher = app.autolaunch();
    if launcher.is_enabled().unwrap_or(false) != general.autostart {
        let result = if general.autostart {
            launcher.enable()
        } else {
            launcher.disable()
        };
        result.map_err(|e| format!("ログイン時の起動を変えられませんでした: {e}"))?;
    }
    Ok(current(&app, &shared))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_menu_scales_are_valid() {
        let ok = General::of(&Settings::default(), false);
        assert!(ok.valid());
        for scale in SCALES {
            assert!(General { scale, ..ok }.valid());
        }
        assert!(!General { scale: 3.0, ..ok }.valid());
        assert!(!General { scale: 0.0, ..ok }.valid());
    }

    #[test]
    fn json_roundtrip_uses_lowercase_speed() {
        let g = General {
            speed: Speed::Fast,
            ..General::of(&Settings::default(), true)
        };
        let json = serde_json::to_string(&g).unwrap();
        assert!(json.contains("\"speed\":\"fast\""));
        assert!(json.contains("\"autostart\":true"));
        assert_eq!(serde_json::from_str::<General>(&json).unwrap(), g);
    }
}
