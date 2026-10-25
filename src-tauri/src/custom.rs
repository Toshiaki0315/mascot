//! 「じぶんの絵」：1 つだけ、自分の画像をキャラクターにできる。
//! 画像は、設定の画面で選んだものを、画面側が小さな PNG にして（`data:` の文字列として）渡してくる。
//! 大きいので、`settings.json` には入れず、設定のフォルダに別のファイルとして置く

use std::{fs, path::PathBuf, sync::Arc};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::{emit_settings, settings::Character, tray, Shared, MAIN_WINDOW};

const FILE: &str = "custom-character.txt";
const PREFIX: &str = "data:image/png;base64,";
/// 受け取る文字列の長さの上限（PNG にして、約 1.5MB）。画面側で小さくするので、ふつうはその 1 割ほど
const MAX_LEN: usize = 2_000_000;

/// 画面側から受け取った文字列が、PNG の `data:` URL として妥当か。
/// 中身は、そのままページに渡して描くので、PNG と base64 の文字だけであることを確かめる
pub fn valid_data_url(s: &str) -> bool {
    let Some(body) = s.strip_prefix(PREFIX) else {
        return false;
    };
    s.len() <= MAX_LEN
        && !body.is_empty()
        && body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(FILE))
}

/// 画像が保存されているか
pub fn exists(app: &AppHandle) -> bool {
    path(app).is_some_and(|p| p.is_file())
}

fn read(app: &AppHandle) -> Option<String> {
    let text = fs::read_to_string(path(app)?).ok()?;
    valid_data_url(&text).then_some(text)
}

/// 画面側へ、画像が変わったことを知らせる（読み直してもらう）
fn notify_changed(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.emit("mascot-custom-image", ());
    }
}

#[tauri::command]
pub fn get_custom_image(app: AppHandle) -> Option<String> {
    read(&app)
}

/// 画像を保存して、キャラクターを「じぶんの絵」に切り替える
#[tauri::command]
pub fn set_custom_image(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    data_url: String,
) -> Result<(), String> {
    if !valid_data_url(&data_url) {
        return Err("画像を読み込めませんでした（PNG に変えられませんでした）".into());
    }
    let file = path(&app).ok_or("設定のフォルダが見つかりません")?;
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // いったん別のファイルに書いてから置き換える（途中で終わっても、前の画像が壊れない）
    let tmp = file.with_extension("txt.tmp");
    fs::write(&tmp, data_url).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &file).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })?;
    shared.store.update(|s| s.character = Character::Custom);
    after_change(&app, &shared);
    Ok(())
}

/// 画像を消す。「じぶんの絵」を選んでいたら、茶トラに戻す
#[tauri::command]
pub fn clear_custom_image(app: AppHandle, shared: State<Arc<Shared>>) {
    if let Some(file) = path(&app) {
        let _ = fs::remove_file(file);
    }
    if shared.store.get().character == Character::Custom {
        shared.store.update(|s| s.character = Character::Tora);
    }
    after_change(&app, &shared);
}

fn after_change(app: &AppHandle, shared: &Shared) {
    tray::sync_characters(app);
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        emit_settings(&window, shared);
    }
    notify_changed(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_png_data_url() {
        assert!(valid_data_url("data:image/png;base64,iVBORw0KGgo="));
        assert!(valid_data_url("data:image/png;base64,AAAA+/9z=="));
    }

    #[test]
    fn rejects_other_kinds_and_odd_characters() {
        assert!(!valid_data_url(""));
        assert!(!valid_data_url("data:image/png;base64,"));
        assert!(!valid_data_url("data:image/jpeg;base64,AAAA"));
        assert!(!valid_data_url("https://example.com/a.png"));
        // ページにそのまま渡すので、base64 以外の文字（引用符や空白、改行）は通さない
        for odd in ["\"", " ", "\n", "\\"] {
            assert!(!valid_data_url(&format!("{PREFIX}AA{odd}AA")), "{odd:?}");
        }
        assert!(!valid_data_url("data:image/png;base64,AA<script>"));
    }

    #[test]
    fn rejects_too_long_strings() {
        let ok = format!("{PREFIX}{}", "A".repeat(MAX_LEN - PREFIX.len()));
        assert!(valid_data_url(&ok));
        let too_long = format!("{ok}A");
        assert!(!valid_data_url(&too_long));
    }
}
