//! 各機能の設定（音を鳴らすか、メニューバーに残り時間を出すか、タイマーの秒読み）。
//! リマインダー / タイマー / ポモドーロの画面から変える

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    settings::{CountdownMode, Settings},
    Shared,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Options {
    /// 時刻になったときに音を鳴らすか
    pub reminder_sound: bool,
    pub timer_sound: bool,
    pub pomodoro_sound: bool,
    /// メニューバーに、残り時間を出すか
    pub menubar_timer: bool,
    pub menubar_pomodoro: bool,
    /// タイマーの最後の秒読み（10 → 1）をふきだしに出すか
    pub countdown_mode: CountdownMode,
}

impl Options {
    pub fn of(s: &Settings) -> Self {
        Self {
            reminder_sound: s.reminder_sound,
            timer_sound: s.timer_sound,
            pomodoro_sound: s.pomodoro_sound,
            menubar_timer: s.menubar_timer,
            menubar_pomodoro: s.menubar_pomodoro,
            countdown_mode: s.countdown_mode,
        }
    }

    pub fn apply(self, s: &mut Settings) {
        s.reminder_sound = self.reminder_sound;
        s.timer_sound = self.timer_sound;
        s.pomodoro_sound = self.pomodoro_sound;
        s.menubar_timer = self.menubar_timer;
        s.menubar_pomodoro = self.menubar_pomodoro;
        s.countdown_mode = self.countdown_mode;
    }
}

#[tauri::command]
pub fn get_options(shared: State<Arc<Shared>>) -> Options {
    Options::of(&shared.store.get())
}

#[tauri::command]
pub fn set_options(shared: State<Arc<Shared>>, options: Options) {
    shared.store.update(|s| options.apply(s));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_every_sound_and_display_on_and_random_countdown() {
        let o = Options::of(&Settings::default());
        assert!(o.reminder_sound && o.timer_sound && o.pomodoro_sound);
        assert!(o.menubar_timer && o.menubar_pomodoro);
        assert_eq!(o.countdown_mode, CountdownMode::Random);
    }

    #[test]
    fn apply_changes_only_the_options() {
        let mut s = Settings::default();
        let o = Options {
            timer_sound: false,
            menubar_pomodoro: false,
            countdown_mode: CountdownMode::Never,
            ..Options::of(&s)
        };
        o.apply(&mut s);
        assert!(!s.timer_sound && !s.menubar_pomodoro);
        assert!(s.reminder_sound && s.pomodoro_sound && s.menubar_timer);
        assert_eq!(s.countdown_mode, CountdownMode::Never);
        assert!(s.walking && s.chatty);
    }

    #[test]
    fn json_uses_snake_case_names_and_lowercase_modes() {
        let json = serde_json::to_string(&Options::of(&Settings::default())).unwrap();
        assert!(json.contains("\"timer_sound\":true"));
        assert!(json.contains("\"countdown_mode\":\"random\""));
        let back: Options = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Options::of(&Settings::default()));
    }
}
