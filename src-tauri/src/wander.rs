//! マスコットの「頭脳」スレッド。歩き回る・眠る・全画面アプリの上では隠れる、を決める

use std::{
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{geometry, system, Shared, MAIN_WINDOW};

const TICK: Duration = Duration::from_millis(16);
/// 画面の抜き差し・全画面の確認は重いので間引く（約 0.5 秒ごと）
const SLOW_CHECK_EVERY: u32 = 30;
/// 眠る・起きるの判定（約 0.1 秒ごと）
const SLEEP_CHECK_EVERY: u32 = 6;
/// 目的地に着いてから次に歩き出すまでの待ち時間（ミリ秒）
const IDLE_MIN_MS: u64 = 2000;
const IDLE_MAX_MS: u64 = 6000;
/// 次の目的地を別の画面にする確率（マルチモニター時）
const OTHER_SCREEN_CHANCE: f64 = 0.25;
/// キーボード・マウスの入力がこの秒数ないと眠る
const SLEEP_AFTER_SECS: f64 = 60.0;

/// フロントエンドへ送る移動状態。dir は進行方向（-1: 左, 1: 右）
#[derive(Clone, Copy, Serialize)]
struct MovePayload {
    moving: bool,
    dir: i8,
}

#[derive(Clone, Copy, Serialize)]
struct FlagPayload {
    on: bool,
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

/// 作業領域（メニューバーと Dock を除く）内で、ランダムな目的地を選ぶ。
/// 画面が複数あるときは、ときどき別の画面へ遠出する
fn pick_target(window: &WebviewWindow, rng: &mut Rng) -> Option<(f64, f64)> {
    let rect = geometry::window_rect(window)?;
    let screens = geometry::screens(window);
    let here = screens
        .iter()
        .position(|s| s.full.contains(rect.center()))
        .unwrap_or(0);
    let index = if screens.len() > 1 && rng.next_f64() < OTHER_SCREEN_CHANCE {
        let other = (rng.next_f64() * (screens.len() - 1) as f64) as usize;
        if other >= here {
            other + 1
        } else {
            other
        }
    } else {
        here
    };
    let work = screens.get(index)?.work;
    Some((
        work.x + rng.range(0.0, (work.w - rect.w).max(0.0)),
        work.y + rng.range(0.0, (work.h - rect.h).max(0.0)),
    ))
}

struct Brain {
    window: WebviewWindow,
    shared: Arc<Shared>,
    rng: Rng,
    target: Option<(f64, f64)>,
    current: Option<(f64, f64)>,
    idle_until: Instant,
    moving: bool,
    sleeping: bool,
    suppressed: bool,
    was_hovering: bool,
}

impl Brain {
    fn set_moving(&mut self, value: bool, dir: i8) {
        if self.moving != value {
            self.moving = value;
            let _ = self
                .window
                .emit("mascot-move", MovePayload { moving: value, dir });
        }
    }

    fn stop_walking(&mut self) {
        self.set_moving(false, 0);
        self.current = None;
        self.target = None;
    }

    fn save_position(&self) {
        if let Some(rect) = geometry::window_rect(&self.window) {
            self.shared
                .store
                .update(|s| s.position = Some((rect.x, rect.y)));
        }
    }

    /// モニターの抜き差しと、全画面アプリの有無を確認する
    fn slow_check(&mut self) {
        if let Some(pos) = geometry::bring_back_if_lost(&self.window) {
            self.stop_walking();
            self.shared.store.update(|s| s.position = Some(pos));
        }

        let hide_on_fullscreen = self.shared.store.get().hide_on_fullscreen;
        let covered = hide_on_fullscreen
            && geometry::window_rect(&self.window)
                .and_then(|rect| {
                    geometry::screens(&self.window)
                        .into_iter()
                        .find(|s| s.full.contains(rect.center()))
                })
                .is_some_and(|screen| system::is_screen_covered(screen.full));

        if covered != self.suppressed {
            self.suppressed = covered;
            // window.hide()/show() はフォーカスを奪うことがあるため、中身を隠すだけにする
            let _ = self
                .window
                .emit("mascot-suppress", FlagPayload { on: covered });
            if covered {
                self.shared.hovering.store(false, Ordering::Relaxed);
                let _ = self.window.set_ignore_cursor_events(true);
            }
        }
    }

    fn check_sleep(&mut self) {
        let sleepy = system::seconds_since_last_input() >= SLEEP_AFTER_SECS;
        if sleepy != self.sleeping {
            self.sleeping = sleepy;
            let _ = self.window.emit("mascot-sleep", FlagPayload { on: sleepy });
        }
    }

    fn resync(&mut self) {
        let _ = self.window.emit(
            "mascot-move",
            MovePayload {
                moving: self.moving,
                dir: 0,
            },
        );
        let _ = self
            .window
            .emit("mascot-sleep", FlagPayload { on: self.sleeping });
        let _ = self.window.emit(
            "mascot-suppress",
            FlagPayload {
                on: self.suppressed,
            },
        );
    }

    fn step(&mut self) {
        let settings = self.shared.store.get();
        let hovering = self.shared.hovering.load(Ordering::Relaxed);

        // ドラッグで動かされたかもしれないので、カーソルが離れたら位置を保存する
        if self.was_hovering && !hovering {
            self.save_position();
        }
        self.was_hovering = hovering;

        if self.shared.reset.swap(false, Ordering::Relaxed) {
            self.stop_walking();
        }

        let paused = hovering
            || self.sleeping
            || self.suppressed
            || !settings.walking
            || !self.window.is_visible().unwrap_or(false);
        if paused {
            self.stop_walking();
            self.idle_until = Instant::now() + Duration::from_millis(IDLE_MIN_MS);
            return;
        }

        let Some(goal) = self.target else {
            if Instant::now() >= self.idle_until {
                self.target = pick_target(&self.window, &mut self.rng);
            }
            return;
        };

        let pos = match self.current {
            Some(p) => p,
            None => match geometry::window_rect(&self.window) {
                Some(r) => (r.x, r.y),
                None => return,
            },
        };

        let step = settings.speed.px_per_sec() * settings.scale * TICK.as_secs_f64();
        let (dx, dy) = (goal.0 - pos.0, goal.1 - pos.1);
        let dist = dx.hypot(dy);

        if dist <= step {
            geometry::move_window(&self.window, goal.0, goal.1);
            self.stop_walking();
            self.shared.store.update(|s| s.position = Some(goal));
            let idle = self.rng.range(IDLE_MIN_MS as f64, IDLE_MAX_MS as f64) as u64;
            self.idle_until = Instant::now() + Duration::from_millis(idle);
            return;
        }

        self.set_moving(true, if dx < 0.0 { -1 } else { 1 });
        let next = (pos.0 + dx / dist * step, pos.1 + dy / dist * step);
        geometry::move_window(&self.window, next.0, next.1);
        self.current = Some(next);
    }
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>) {
    thread::spawn(move || {
        let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
            return;
        };
        let mut brain = Brain {
            window,
            shared,
            rng: Rng::new(),
            target: None,
            current: None,
            idle_until: Instant::now() + Duration::from_millis(IDLE_MIN_MS),
            moving: false,
            sleeping: false,
            suppressed: false,
            was_hovering: false,
        };
        let mut ticks: u32 = 0;
        loop {
            thread::sleep(TICK);
            if ticks.is_multiple_of(SLOW_CHECK_EVERY) {
                brain.slow_check();
            }
            if ticks.is_multiple_of(SLEEP_CHECK_EVERY) {
                brain.check_sleep();
            }
            ticks = ticks.wrapping_add(1);
            if brain.shared.resync.swap(false, Ordering::Relaxed) {
                brain.resync();
            }
            brain.step();
        }
    });
}
