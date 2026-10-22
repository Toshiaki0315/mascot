//! メモ：一言のメモを預けておき、あとでクリップボードにコピーして貼り付ける

use std::{
    io::Write,
    process::{Command, Stdio},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{reminder::now_ms, tray, Shared};

/// 預けられるメモの長さと件数
pub const MAX_CHARS: usize = 300;
const MAX_NOTES: usize = 30;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub text: String,
    /// 預けた時刻（UNIX エポックからのミリ秒）
    pub at: i64,
}

/// 新しい順
fn newest_first(mut list: Vec<Note>) -> Vec<Note> {
    list.sort_by_key(|n| std::cmp::Reverse((n.at, n.id)));
    list
}

/// 日本語が文字化けしないよう、pbcopy / pbpaste には UTF-8 を指定する
/// （Finder などから起動したアプリは、ロケールが C になっていることがある）
const LOCALE: &str = "en_US.UTF-8";

pub fn copy_to_clipboard(text: &str) -> bool {
    let Ok(mut child) = Command::new("pbcopy")
        .env("LANG", LOCALE)
        .stdin(Stdio::piped())
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
    child.wait().is_ok() && wrote
}

fn read_clipboard() -> Option<String> {
    let output = Command::new("pbpaste").env("LANG", LOCALE).output().ok()?;
    String::from_utf8(output.stdout).ok()
}

fn add(app: &AppHandle, shared: &Shared, text: &str) -> Result<Vec<Note>, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("メモが空です".into());
    }
    // 長すぎるものは、切り詰めて預ける
    let text: String = text.chars().take(MAX_CHARS).collect();
    shared.store.update(|s| {
        let id = s.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        s.notes.push(Note {
            id,
            text,
            at: now_ms(),
        });
        // 多すぎたら、古いものから消す
        let mut sorted = newest_first(std::mem::take(&mut s.notes));
        sorted.truncate(MAX_NOTES);
        s.notes = sorted;
    });
    tray::refresh_notes(app);
    Ok(newest_first(shared.store.get().notes))
}

#[tauri::command]
pub fn list_notes(shared: State<Arc<Shared>>) -> Vec<Note> {
    newest_first(shared.store.get().notes)
}

#[tauri::command]
pub fn add_note(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    text: String,
) -> Result<Vec<Note>, String> {
    add(&app, &shared, &text)
}

#[tauri::command]
pub fn delete_note(app: AppHandle, shared: State<Arc<Shared>>, id: u64) -> Vec<Note> {
    shared.store.update(|s| s.notes.retain(|n| n.id != id));
    tray::refresh_notes(&app);
    newest_first(shared.store.get().notes)
}

/// 預けたメモを、クリップボードにコピーする
pub fn copy_note(shared: &Shared, id: u64) -> bool {
    shared
        .store
        .get()
        .notes
        .iter()
        .find(|n| n.id == id)
        .is_some_and(|n| copy_to_clipboard(&n.text))
}

/// 預けたメモを、クリップボードにコピーする（メモの画面のボタン用）
#[tauri::command]
pub fn copy_note_to_clipboard(shared: State<Arc<Shared>>, id: u64) -> bool {
    copy_note(&shared, id)
}

/// いまのクリップボードの文字を、メモに預ける（メモの画面のボタン用）
#[tauri::command]
pub fn save_clipboard_note(
    app: AppHandle,
    shared: State<Arc<Shared>>,
) -> Result<Vec<Note>, String> {
    save_clipboard(&app, &shared)
}

/// いまのクリップボードの文字を、メモに預ける
pub fn save_clipboard(app: &AppHandle, shared: &Shared) -> Result<Vec<Note>, String> {
    let text = read_clipboard().ok_or("クリップボードを読めませんでした")?;
    add(app, shared, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(id: u64, at: i64) -> Note {
        Note {
            id,
            text: format!("n{id}"),
            at,
        }
    }

    #[test]
    fn newest_first_orders_by_time_then_id() {
        let ids: Vec<_> = newest_first(vec![n(1, 100), n(2, 300), n(3, 300), n(4, 200)])
            .iter()
            .map(|n| n.id)
            .collect();
        assert_eq!(ids, vec![3, 2, 4, 1]);
    }
}
