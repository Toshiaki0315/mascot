//! メニューバーのトレイアイコンと設定メニュー

use std::sync::{atomic::Ordering, Arc, Mutex};

use chrono::{Datelike, Local, TimeZone, Timelike};

use tauri::{
    image::Image,
    menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewWindow, Wry,
};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    about, apply_scale, emit_settings,
    reminder::{self, Reminder, Repeat},
    save_current_position,
    settings::{Character, Speed, SCALES},
    Shared, MAIN_WINDOW,
};

/// メニュー項目のチェック状態を後から書き換えるために持っておく
struct TrayItems {
    walking: CheckMenuItem<Wry>,
    speeds: Vec<(Speed, CheckMenuItem<Wry>)>,
    scales: Vec<(f64, CheckMenuItem<Wry>)>,
    characters: Vec<(Character, CheckMenuItem<Wry>)>,
    fullscreen: CheckMenuItem<Wry>,
    chatty: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    reminder_sound: CheckMenuItem<Wry>,
    /// 「リマインダー」のサブメニューと、その中の直近の予定の行（予定が変わるたびに作り直す）
    reminder_menu: Submenu<Wry>,
    reminder_rows: Mutex<Vec<MenuItem<Wry>>>,
    /// トレイと、マスコットの右クリックで共有する
    menu: Menu<Wry>,
}

fn scale_id(scale: f64) -> String {
    format!("scale-{scale}")
}

pub fn setup(app: &tauri::App, shared: &Arc<Shared>) -> tauri::Result<()> {
    let settings = shared.store.get();
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);

    let about = MenuItem::with_id(
        app,
        "about",
        format!("{} について", app.package_info().name),
        true,
        None::<&str>,
    )?;
    let toggle = MenuItem::with_id(app, "toggle", "表示 / 非表示", true, None::<&str>)?;
    let manage_reminders = MenuItem::with_id(
        app,
        "reminders",
        "リマインダーの追加・編集…",
        true,
        None::<&str>,
    )?;
    let reminder_menu = Submenu::with_items(
        app,
        "リマインダー",
        true,
        &[&manage_reminders, &PredefinedMenuItem::separator(app)?],
    )?;
    let walking = CheckMenuItem::with_id(
        app,
        "walking",
        "歩き回る",
        true,
        settings.walking,
        None::<&str>,
    )?;

    let speeds = Speed::ALL
        .iter()
        .map(|&speed| {
            CheckMenuItem::with_id(
                app,
                speed.id(),
                speed.label(),
                true,
                settings.speed == speed,
                None::<&str>,
            )
            .map(|item| (speed, item))
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let speed_menu = Submenu::with_items(
        app,
        "歩く速さ",
        true,
        &speeds
            .iter()
            .map(|(_, item)| item as &dyn tauri::menu::IsMenuItem<Wry>)
            .collect::<Vec<_>>(),
    )?;

    let scales = SCALES
        .iter()
        .map(|&scale| {
            CheckMenuItem::with_id(
                app,
                scale_id(scale),
                format!("{scale} 倍"),
                true,
                settings.scale == scale,
                None::<&str>,
            )
            .map(|item| (scale, item))
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let scale_menu = Submenu::with_items(
        app,
        "大きさ",
        true,
        &scales
            .iter()
            .map(|(_, item)| item as &dyn tauri::menu::IsMenuItem<Wry>)
            .collect::<Vec<_>>(),
    )?;

    let characters = Character::ALL
        .iter()
        .map(|&character| {
            CheckMenuItem::with_id(
                app,
                character.id(),
                character.label(),
                true,
                settings.character == character,
                None::<&str>,
            )
            .map(|item| (character, item))
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let character_menu = Submenu::with_items(
        app,
        "キャラクター",
        true,
        &characters
            .iter()
            .map(|(_, item)| item as &dyn tauri::menu::IsMenuItem<Wry>)
            .collect::<Vec<_>>(),
    )?;

    let chatty = CheckMenuItem::with_id(
        app,
        "chatty",
        "おしゃべり",
        true,
        settings.chatty,
        None::<&str>,
    )?;
    let reminder_sound = CheckMenuItem::with_id(
        app,
        "reminder-sound",
        "リマインダーの音",
        true,
        settings.reminder_sound,
        None::<&str>,
    )?;
    let fullscreen = CheckMenuItem::with_id(
        app,
        "hide-on-fullscreen",
        "全画面アプリの上では隠れる",
        true,
        settings.hide_on_fullscreen,
        None::<&str>,
    )?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "ログイン時に起動",
        true,
        autostart_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &reminder_menu,
            &PredefinedMenuItem::separator(app)?,
            &character_menu,
            &walking,
            &speed_menu,
            &scale_menu,
            &chatty,
            &reminder_sound,
            &fullscreen,
            &PredefinedMenuItem::separator(app)?,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    app.manage(TrayItems {
        walking,
        speeds,
        scales,
        characters,
        fullscreen,
        chatty,
        autostart,
        reminder_sound,
        reminder_menu,
        reminder_rows: Mutex::new(Vec::new()),
        menu: menu.clone(),
    });

    // 白黒のテンプレート画像にすると、ライト/ダークどちらのメニューバーにもなじむ
    let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;
    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Desktop Mascot")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .build(app)?;
    refresh_reminders(app.handle());
    Ok(())
}

/// メニューに出す直近のリマインダーの件数
const UPCOMING: usize = 5;
/// メニューの 1 行に出す内容の文字数
const LABEL_CHARS: usize = 14;

fn reminder_label(r: &Reminder) -> String {
    let when = match Local.timestamp_millis_opt(r.at).single() {
        Some(dt) => format!(
            "{}/{} {:02}:{:02}",
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute()
        ),
        None => String::new(),
    };
    let mark = if r.repeat == Repeat::Once { "" } else { " ↻" };
    let chars: Vec<char> = r.text.chars().collect();
    let text = if chars.len() > LABEL_CHARS {
        format!("{}…", chars[..LABEL_CHARS - 1].iter().collect::<String>())
    } else {
        r.text.clone()
    };
    format!("{when}{mark}  {text}")
}

/// 「リマインダー」サブメニューの、直近の予定の行を作り直す。
/// 予定を足した・直した・消した・知らせたときに呼ぶ
pub fn refresh_reminders(app: &AppHandle) {
    let (Some(items), Some(shared)) =
        (app.try_state::<TrayItems>(), app.try_state::<Arc<Shared>>())
    else {
        return;
    };
    let mut list = shared.store.get().reminders;
    list.sort_by_key(|r| (r.at, r.id));

    let mut rows = items.reminder_rows.lock().unwrap();
    for row in rows.drain(..) {
        let _ = items.reminder_menu.remove(&row);
    }
    let mut new_rows = Vec::new();
    if list.is_empty() {
        new_rows.extend(MenuItem::with_id(
            app,
            "reminders-none",
            "予定はありません",
            false,
            None::<&str>,
        ));
    }
    for r in list.iter().take(UPCOMING) {
        // 押すと、追加・編集のウィンドウが開く
        new_rows.extend(MenuItem::with_id(
            app,
            format!("reminders-item-{}", r.id),
            reminder_label(r),
            true,
            None::<&str>,
        ));
    }
    if list.len() > UPCOMING {
        new_rows.extend(MenuItem::with_id(
            app,
            "reminders-more",
            format!("ほか {} 件", list.len() - UPCOMING),
            false,
            None::<&str>,
        ));
    }
    for row in new_rows {
        if items.reminder_menu.append(&row).is_ok() {
            rows.push(row);
        }
    }
}

/// マスコットの上に、トレイと同じメニューを出す
pub fn popup(window: &WebviewWindow) {
    let items = window.app_handle().state::<TrayItems>();
    let _ = items.menu.popup(window.as_ref().window());
}

fn on_menu(app: &AppHandle, id: &str) {
    let shared = app.state::<Arc<Shared>>();
    let items = app.state::<TrayItems>();
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    match id {
        "about" => {
            let info = app.package_info();
            about::show(&info.name, &info.version.to_string());
        }
        id if id.starts_with("reminders") => reminder::open_window(app),
        "toggle" => {
            let visible = !window.is_visible().unwrap_or(false);
            let _ = if visible {
                window.show()
            } else {
                window.hide()
            };
            shared.visible.store(visible, Ordering::Relaxed);
            shared.store.update(|s| s.visible = visible);
        }
        "walking" => {
            let on = items.walking.is_checked().unwrap_or(true);
            shared.store.update(|s| s.walking = on);
        }
        "chatty" => {
            let on = items.chatty.is_checked().unwrap_or(true);
            shared.store.update(|s| s.chatty = on);
            emit_settings(&window, &shared);
        }
        "reminder-sound" => {
            let on = items.reminder_sound.is_checked().unwrap_or(true);
            shared.store.update(|s| s.reminder_sound = on);
        }
        "hide-on-fullscreen" => {
            let on = items.fullscreen.is_checked().unwrap_or(true);
            shared.store.update(|s| s.hide_on_fullscreen = on);
        }
        "autostart" => {
            let on = items.autostart.is_checked().unwrap_or(false);
            let launcher = app.autolaunch();
            let result = if on {
                launcher.enable()
            } else {
                launcher.disable()
            };
            // 登録に失敗したらチェックを実際の状態に戻す
            if result.is_err() {
                let _ = items
                    .autostart
                    .set_checked(launcher.is_enabled().unwrap_or(false));
            }
        }
        "quit" => {
            save_current_position(&window, &shared);
            app.exit(0);
        }
        _ => {
            // 速さ・大きさ・キャラクターはラジオボタンのように 1 つだけチェックする
            if let Some(&(character, _)) = items.characters.iter().find(|(c, _)| c.id() == id) {
                for (c, item) in &items.characters {
                    let _ = item.set_checked(*c == character);
                }
                shared.store.update(|s| s.character = character);
                emit_settings(&window, &shared);
            } else if let Some(&(speed, _)) = items.speeds.iter().find(|(s, _)| s.id() == id) {
                for (s, item) in &items.speeds {
                    let _ = item.set_checked(*s == speed);
                }
                shared.store.update(|s| s.speed = speed);
            } else if let Some(&(scale, _)) = items.scales.iter().find(|(s, _)| scale_id(*s) == id)
            {
                for (s, item) in &items.scales {
                    let _ = item.set_checked(*s == scale);
                }
                apply_scale(&window, scale);
                shared.store.update(|s| s.scale = scale);
                save_current_position(&window, &shared);
            }
        }
    }
}
