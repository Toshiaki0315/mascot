//! ポモドーロ：作業と休憩をくり返すタイマー（状態の遷移、設定、開始・停止の IPC）

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{reminder::now_ms, settings::Settings, tray, Shared};

const MIN_MS: i64 = 60_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Work,
    Break,
    /// 何回かに 1 回の、長い休憩
    Longbreak,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Work => "作業中",
            Phase::Break => "休憩中",
            Phase::Longbreak => "長い休憩中",
        }
    }
}

/// 動いているポモドーロ
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Pomodoro {
    pub phase: Phase,
    /// いまの区間が終わる時刻（UNIX エポックからのミリ秒）
    pub ends_at: i64,
    /// 終わった作業の回数
    pub cycle: u32,
}

/// 区間の長さ（分）。長い休憩は `long_every` 回の作業ごとに 1 回
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PomodoroCfg {
    pub work_min: u32,
    pub break_min: u32,
    pub long_break_min: u32,
    pub long_every: u32,
}

impl Default for PomodoroCfg {
    fn default() -> Self {
        Self {
            work_min: 25,
            break_min: 5,
            long_break_min: 15,
            long_every: 4,
        }
    }
}

impl PomodoroCfg {
    fn valid(&self) -> bool {
        let minutes = 1..=180;
        minutes.contains(&self.work_min)
            && minutes.contains(&self.break_min)
            && minutes.contains(&self.long_break_min)
            && (1..=12).contains(&self.long_every)
    }
}

pub fn start(cfg: &PomodoroCfg, now: i64) -> Pomodoro {
    Pomodoro {
        phase: Phase::Work,
        ends_at: now + i64::from(cfg.work_min) * MIN_MS,
        cycle: 0,
    }
}

/// いまの区間が終わった時に、次の区間と、知らせる文を返す。
/// アプリを止めていて何区間分も過ぎていても、進めるのは 1 区間だけ（`now` から数え直す）
pub fn advance(p: &Pomodoro, cfg: &PomodoroCfg, now: i64) -> (Pomodoro, String) {
    match p.phase {
        Phase::Work => {
            let cycle = p.cycle + 1;
            let (phase, min) = if cycle.is_multiple_of(cfg.long_every) {
                (Phase::Longbreak, cfg.long_break_min)
            } else {
                (Phase::Break, cfg.break_min)
            };
            let text = if phase == Phase::Longbreak {
                format!("{cycle} 回おつかれさま！ {min} 分ゆっくり休もう")
            } else {
                format!("作業おわり！ {min} 分休憩しよう")
            };
            (
                Pomodoro {
                    phase,
                    ends_at: now + i64::from(min) * MIN_MS,
                    cycle,
                },
                text,
            )
        }
        Phase::Break | Phase::Longbreak => (
            Pomodoro {
                phase: Phase::Work,
                ends_at: now + i64::from(cfg.work_min) * MIN_MS,
                cycle: p.cycle,
            },
            format!("休憩おわり！ 次の作業（{} 分）をはじめよう", cfg.work_min),
        ),
    }
}

/// 区間が終わっていれば進めて、知らせる文を返す（スケジューラから呼ぶ）
pub fn tick(app: &AppHandle, shared: &Shared, now: i64) -> Option<String> {
    let settings = shared.store.get();
    let p = settings.pomodoro.filter(|p| p.ends_at <= now)?;
    let (next, text) = advance(&p, &settings.pomodoro_cfg, now);
    shared.store.update(|s| s.pomodoro = Some(next));
    tray::refresh_timers(app);
    Some(text)
}

#[tauri::command]
pub fn pomodoro_status(shared: State<Arc<Shared>>) -> (Option<Pomodoro>, PomodoroCfg) {
    let s: Settings = shared.store.get();
    (s.pomodoro, s.pomodoro_cfg)
}

#[tauri::command]
pub fn pomodoro_start(app: AppHandle, shared: State<Arc<Shared>>) {
    start_now(&app, &shared);
}

pub fn start_now(app: &AppHandle, shared: &Shared) {
    let cfg = shared.store.get().pomodoro_cfg;
    shared
        .store
        .update(|s| s.pomodoro = Some(start(&cfg, now_ms())));
    tray::refresh_timers(app);
}

#[tauri::command]
pub fn pomodoro_stop(app: AppHandle, shared: State<Arc<Shared>>) {
    stop_now(&app, &shared);
}

pub fn stop_now(app: &AppHandle, shared: &Shared) {
    shared.store.update(|s| s.pomodoro = None);
    tray::refresh_timers(app);
}

#[tauri::command]
pub fn set_pomodoro_config(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    cfg: PomodoroCfg,
) -> Result<(), String> {
    if !cfg.valid() {
        return Err("時間は 1〜180 分、長い休憩の間隔は 1〜12 回にしてください".into());
    }
    shared.store.update(|s| s.pomodoro_cfg = cfg);
    tray::refresh_timers(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_000_000;

    #[test]
    fn start_begins_with_work() {
        let p = start(&PomodoroCfg::default(), T0);
        assert_eq!(p.phase, Phase::Work);
        assert_eq!(p.ends_at, T0 + 25 * MIN_MS);
        assert_eq!(p.cycle, 0);
    }

    #[test]
    fn work_then_break_then_work() {
        let cfg = PomodoroCfg::default();
        let mut p = start(&cfg, T0);
        let now = p.ends_at;
        let (after_work, text) = advance(&p, &cfg, now);
        assert_eq!(after_work.phase, Phase::Break);
        assert_eq!(after_work.cycle, 1);
        assert_eq!(after_work.ends_at, now + 5 * MIN_MS);
        assert!(text.contains("5 分休憩"));
        p = after_work;
        let (after_break, text) = advance(&p, &cfg, p.ends_at);
        assert_eq!(after_break.phase, Phase::Work);
        assert_eq!(after_break.cycle, 1);
        assert!(text.contains("25 分"));
    }

    #[test]
    fn every_fourth_work_gets_a_long_break() {
        let cfg = PomodoroCfg::default();
        let mut p = start(&cfg, T0);
        let mut phases = Vec::new();
        for _ in 0..8 {
            let (next, _) = advance(&p, &cfg, p.ends_at);
            phases.push(next.phase);
            p = next;
        }
        assert_eq!(
            phases,
            [
                Phase::Break,
                Phase::Work,
                Phase::Break,
                Phase::Work,
                Phase::Break,
                Phase::Work,
                Phase::Longbreak,
                Phase::Work
            ]
        );
        assert_eq!(p.cycle, 4);
    }

    #[test]
    fn a_long_absence_advances_one_phase_from_now() {
        let cfg = PomodoroCfg::default();
        let p = start(&cfg, T0);
        let now = p.ends_at + 10 * 60 * MIN_MS;
        let (next, _) = advance(&p, &cfg, now);
        assert_eq!(next.ends_at, now + 5 * MIN_MS);
    }

    #[test]
    fn config_validation() {
        assert!(PomodoroCfg::default().valid());
        assert!(!PomodoroCfg {
            work_min: 0,
            ..PomodoroCfg::default()
        }
        .valid());
        assert!(!PomodoroCfg {
            long_every: 13,
            ..PomodoroCfg::default()
        }
        .valid());
    }

    #[test]
    fn missing_config_fields_use_defaults() {
        let cfg: PomodoroCfg = serde_json::from_str(r#"{"work_min":50}"#).unwrap();
        assert_eq!(cfg.work_min, 50);
        assert_eq!(cfg.break_min, 5);
    }
}
