//! マスコットの「頭脳」スレッド。歩き回る・窓に乗る・投げられて飛ぶ・眠る・
//! 全画面アプリの上では隠れる、を決める

use std::{
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{
    geometry::{self, Rect, Screen},
    physics::{self, Body, Bounds},
    system::{self, WindowInfo},
    Geometry, Shared, BASE_FOOT_OFFSET, BASE_HEAD_HEIGHT, MAIN_WINDOW,
};

/// 飛んでいる・ドラッグ中の間隔。放物線や投げる速さをなめらかに扱うため約 60fps
const FAST_TICK: Duration = Duration::from_millis(16);
/// 歩いている間の間隔。1 コマ 2pt ほどなので、約 30fps でもドット絵らしく見える
const WALK_TICK: Duration = Duration::from_millis(33);
/// じっとしている間の間隔。省電力のため長めにする
const IDLE_TICK: Duration = Duration::from_millis(100);
/// 画面の抜き差し・全画面の確認の間隔
const WINDOW_CHECK: Duration = Duration::from_millis(1000);
/// 窓の上に乗っている間は、窓が動いたり閉じたりしたのを早めに知りたい
const WINDOW_CHECK_PERCHED: Duration = Duration::from_millis(250);
/// 眠る・起きる、休憩の声かけの確認の間隔
const SLEEP_CHECK: Duration = Duration::from_millis(250);

/// 目的地に着いてから次に歩き出すまでの待ち時間（ミリ秒）
const IDLE_MIN_MS: u64 = 2000;
const IDLE_MAX_MS: u64 = 6000;
/// 次の目的地を別の画面にする確率（マルチモニター時）
const OTHER_SCREEN_CHANCE: f64 = 0.25;
/// 地面にいるとき、次はどこかの窓の上に飛び乗る確率
const PERCH_CHANCE: f64 = 0.3;
/// 窓の上にいるとき、窓のふちに沿って歩き続ける確率（残りは飛び降りる）
const STAY_ON_PERCH_CHANCE: f64 = 0.7;
/// 飛び乗る窓の幅は、キャラクターの何倍以上必要か
const PERCH_MIN_WIDTH: f64 = 1.5;
/// これより速く手を離したら「投げた」とみなす（pt/秒）
const THROW_MIN_SPEED: f64 = 600.0;
/// ドラッグ中の位置を何秒分覚えておくか（手を離した時の速さを求める）
const DRAG_SAMPLE_SECS: f64 = 0.1;

/// キーボード・マウスの入力がこの秒数ないと眠る
const SLEEP_AFTER_SECS: f64 = 60.0;
/// この秒数以上入力がなければ「休憩した」とみなし、作業時間を数え直す
const BREAK_RESET_SECS: f64 = 300.0;
/// 休憩せずにこの時間作業を続けたら、休憩をすすめる
const BREAK_REMIND_AFTER: Duration = Duration::from_secs(50 * 60);

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

#[derive(Clone, Copy, Serialize)]
struct SayPayload {
    kind: &'static str,
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

    fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }
}

/// ドラッグ中に記録する (開始からの経過秒, ウィンドウ左上の位置)
type DragSample = (f64, (f64, f64));

/// 乗っている窓
#[derive(Clone, Copy)]
struct Perch {
    window_id: i64,
    bounds: Rect,
}

enum Flight {
    /// 投げられた・落ちている。範囲の端で跳ね返り、床に着地する
    Free { body: Body, bounds: Bounds },
    /// 窓の上へ放物線を描いて飛び乗る
    Jump {
        from: (f64, f64),
        to: (f64, f64),
        vel: (f64, f64),
        started: Instant,
        duration: f64,
        perch: Perch,
    },
}

enum Mode {
    /// 作業領域の中を自由に歩く
    Ground,
    /// 窓の上（ふち）に乗っている
    Perched(Perch),
    Flying(Flight),
}

/// 作業領域（メニューバーと Dock を除く）内で、ランダムな目的地を選ぶ。
/// 画面が複数あるときは、ときどき別の画面へ遠出する
fn pick_ground_target(rect: Rect, screens: &[Screen], rng: &mut Rng) -> Option<(f64, f64)> {
    let here = screens
        .iter()
        .position(|s| s.full.contains(rect.center()))
        .unwrap_or(0);
    let index = if screens.len() > 1 && rng.chance(OTHER_SCREEN_CHANCE) {
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

/// ふちの上で、立てる場所を探す間隔（pt）
const PERCH_SPOT_STEP: f64 = 16.0;

/// 窓のふちに立つときの、キャラクターの寸法（pt）
#[derive(Clone, Copy)]
struct Stance {
    /// ウィンドウの下端から足元まで
    foot_offset: f64,
    /// 足元から耳の先まで
    head_height: f64,
}

impl Stance {
    fn new(scale: f64) -> Self {
        Self {
            foot_offset: BASE_FOOT_OFFSET * scale,
            head_height: BASE_HEAD_HEIGHT * scale,
        }
    }

    /// ふちに立つときのウィンドウ左上の y
    fn window_y(&self, edge_y: f64, rect: Rect) -> f64 {
        edge_y - (rect.h - self.foot_offset)
    }
}

/// 窓のふちの上で、キャラクターが立てる場所（ウィンドウ左上の x）の一覧。
/// 足元が手前の窓に隠れている場所や、画面からはみ出す場所は除く
fn perch_spots(
    windows: &[WindowInfo],
    index: usize,
    rect: Rect,
    screen: &Screen,
    stance: Stance,
) -> Vec<f64> {
    let b = windows[index].bounds;
    let work = screen.work;
    // 頭がメニューバーの下に入らない高さで、画面の下端にも近すぎないこと
    // （頭の上の透明な余白はメニューバーに重なってもよい）
    if b.w < rect.w * PERCH_MIN_WIDTH
        || b.y - stance.head_height < work.y
        || b.y > work.y + work.h - 20.0
    {
        return Vec::new();
    }
    let min = b.x.max(work.x);
    let max = (b.x + b.w).min(work.x + work.w) - rect.w;
    if max < min {
        return Vec::new();
    }
    let count = ((max - min) / PERCH_SPOT_STEP) as usize;
    (0..=count)
        .map(|i| min + i as f64 * PERCH_SPOT_STEP)
        .filter(|&x| !system::is_covered_in_front(windows, index, (x + rect.w / 2.0, b.y + 2.0)))
        .collect()
}

/// 飛び乗れる窓を探す。同じ画面にあって、ふちが見えていて、上に立つ余白がある窓
fn find_perch(
    windows: &[WindowInfo],
    rect: Rect,
    screen: &Screen,
    stance: Stance,
    rng: &mut Rng,
) -> Option<((f64, f64), Perch)> {
    let spots: Vec<(usize, f64)> = windows
        .iter()
        .enumerate()
        .filter(|(_, w)| w.layer == 0 && !w.own)
        .flat_map(|(index, _)| {
            perch_spots(windows, index, rect, screen, stance)
                .into_iter()
                .map(move |x| (index, x))
        })
        .collect();
    if spots.is_empty() {
        return None;
    }
    let (index, x) = spots[(rng.next_f64() * spots.len() as f64) as usize];
    let w = windows[index];
    Some((
        (x, stance.window_y(w.bounds.y, rect)),
        Perch {
            window_id: w.id,
            bounds: w.bounds,
        },
    ))
}

struct Brain {
    window: WebviewWindow,
    shared: Arc<Shared>,
    rng: Rng,
    mode: Mode,
    /// 歩いて向かっている目的地（ウィンドウ左上）
    target: Option<(f64, f64)>,
    /// 頭脳が把握しているウィンドウ左上の位置と大きさ。None なら読み直す。
    /// 歩くたびにメインスレッドへ問い合わせると CPU を食うので覚えておく
    pos: Option<(f64, f64)>,
    size: Option<(f64, f64)>,
    /// カーソル座標の換算に使う、メイン画面の倍率（Geometry::cursor_scale）
    cursor_scale: f64,
    idle_until: Instant,
    moving: bool,
    flying: bool,
    sleeping: bool,
    suppressed: bool,
    /// ドラッグ中の位置の記録（開始時刻と、経過秒ごとの位置）
    drag: Option<(Instant, Vec<DragSample>)>,
    last_tick: Instant,
    last_window_check: Option<Instant>,
    last_sleep_check: Option<Instant>,
    /// 休憩せずに作業を続けている開始時刻
    working_since: Option<Instant>,
}

impl Brain {
    fn new(window: WebviewWindow, shared: Arc<Shared>) -> Self {
        Self {
            window,
            shared,
            rng: Rng::new(),
            mode: Mode::Ground,
            target: None,
            pos: None,
            size: None,
            cursor_scale: 1.0,
            idle_until: Instant::now() + Duration::from_millis(IDLE_MIN_MS),
            moving: false,
            flying: false,
            sleeping: false,
            suppressed: false,
            drag: None,
            last_tick: Instant::now(),
            last_window_check: None,
            last_sleep_check: None,
            working_since: None,
        }
    }

    /// 次の tick までの間隔。動きに合わせて間隔を変え、じっとしている間は CPU を休ませる
    fn interval(&self) -> Duration {
        if self.drag.is_some() || matches!(self.mode, Mode::Flying(_)) {
            FAST_TICK
        } else if self.moving {
            WALK_TICK
        } else {
            IDLE_TICK
        }
    }

    // ---- ウィンドウの位置 ----

    /// 今のウィンドウの位置・大きさ。位置は頭脳の把握している値を優先する
    fn rect(&mut self) -> Option<Rect> {
        if let (Some((x, y)), Some((w, h))) = (self.pos, self.size) {
            return Some(Rect::new(x, y, w, h));
        }
        let actual = geometry::window_rect(&self.window)?;
        let (x, y) = *self.pos.get_or_insert((actual.x, actual.y));
        self.size = Some((actual.w, actual.h));
        let rect = Rect::new(x, y, actual.w, actual.h);
        self.publish(rect);
        Some(rect)
    }

    fn publish(&self, rect: Rect) {
        self.shared.set_geometry(Geometry {
            rect,
            cursor_scale: self.cursor_scale,
        });
    }

    fn move_to(&mut self, pos: (f64, f64)) {
        geometry::move_window(&self.window, pos.0, pos.1);
        self.pos = Some(pos);
        if let Some(mut g) = self.shared.geometry() {
            g.rect.x = pos.0;
            g.rect.y = pos.1;
            self.shared.set_geometry(g);
        }
    }

    fn save_position(&self) {
        if let Some(pos) = self.pos {
            self.shared.store.update(|s| s.position = Some(pos));
        }
    }

    fn stance(&self) -> Stance {
        Stance::new(self.shared.store.get().scale)
    }

    fn screen_of(&self, rect: Rect) -> Option<Screen> {
        let screens = geometry::screens(&self.window);
        let index = screens
            .iter()
            .position(|s| s.full.contains(rect.center()))
            .unwrap_or(0);
        screens.into_iter().nth(index)
    }

    // ---- フロントエンドへの通知 ----

    fn set_moving(&mut self, value: bool, dir: i8) {
        if self.moving != value {
            self.moving = value;
            let _ = self
                .window
                .emit("mascot-move", MovePayload { moving: value, dir });
        }
    }

    fn set_flying(&mut self, value: bool) {
        if self.flying != value {
            self.flying = value;
            let _ = self.window.emit("mascot-fly", FlagPayload { on: value });
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
            .emit("mascot-fly", FlagPayload { on: self.flying });
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

    fn stop_walking(&mut self) {
        self.set_moving(false, 0);
        self.target = None;
    }

    fn rest(&mut self) {
        let idle = self.rng.range(IDLE_MIN_MS as f64, IDLE_MAX_MS as f64) as u64;
        self.idle_until = Instant::now() + Duration::from_millis(idle);
    }

    // ---- 飛ぶ ----

    /// 投げられた・飛び降りた・落ちた。今いる画面の作業領域の中で跳ね回って着地する
    fn start_free_flight(&mut self, vel: (f64, f64)) {
        let Some(rect) = self.rect() else { return };
        let Some(screen) = self.screen_of(rect) else {
            return;
        };
        let work = screen.work;
        let bounds = Bounds {
            min_x: work.x,
            max_x: work.x + (work.w - rect.w).max(0.0),
            min_y: work.y,
            max_y: work.y + (work.h - rect.h).max(0.0),
        };
        self.stop_walking();
        self.mode = Mode::Flying(Flight::Free {
            body: Body {
                pos: (rect.x, rect.y),
                vel,
            },
            bounds,
        });
        self.set_flying(true);
    }

    fn start_jump(&mut self, to: (f64, f64), perch: Perch) {
        let Some(rect) = self.rect() else { return };
        let from = (rect.x, rect.y);
        let distance = (to.0 - from.0).hypot(to.1 - from.1);
        let duration = (distance / 700.0).clamp(0.5, 1.1);
        self.stop_walking();
        self.mode = Mode::Flying(Flight::Jump {
            from,
            to,
            vel: physics::aim(from, to, duration),
            started: Instant::now(),
            duration,
            perch,
        });
        self.set_flying(true);
    }

    fn flight_step(&mut self, dt: f64) {
        let Mode::Flying(flight) = &mut self.mode else {
            return;
        };
        match flight {
            Flight::Free { body, bounds } => {
                let landed = physics::step(body, bounds, dt) == physics::Step::Landed;
                let pos = body.pos;
                self.move_to(pos);
                if landed {
                    self.mode = Mode::Ground;
                    self.set_flying(false);
                    self.save_position();
                    self.rest();
                }
            }
            Flight::Jump {
                from,
                to,
                vel,
                started,
                duration,
                perch,
            } => {
                let t = started.elapsed().as_secs_f64();
                if t >= *duration {
                    let (to, perch) = (*to, *perch);
                    self.move_to(to);
                    self.mode = Mode::Perched(perch);
                    self.set_flying(false);
                    self.save_position();
                    self.rest();
                } else {
                    let pos = physics::position_at(*from, *vel, t);
                    self.move_to(pos);
                }
            }
        }
    }

    // ---- ドラッグ・投げる ----

    /// ドラッグ中なら true。手を離した瞬間の速さが十分なら投げる。
    /// ドラッグ中のウィンドウは macOS が動かすので、その間は実際の位置を読み続ける
    fn track_drag(&mut self, hovering: bool) -> bool {
        // フロントエンドがドラッグを始めた知らせ。カーソルの判定に頼ると、
        // ウィンドウが動いた直後に「カーソルが外れた」と誤判定してドラッグを見逃すことがある
        let started = self.shared.drag_started.swap(false, Ordering::Relaxed);
        let watching = started || hovering || self.drag.is_some();
        let dragging = started || (watching && system::is_left_button_down());
        if dragging {
            if self.drag.is_none() {
                // 飛んでいる最中につかまれたら、そこで止まる
                self.stop_walking();
                self.mode = Mode::Ground;
                self.set_flying(false);
                self.drag = Some((Instant::now(), Vec::new()));
            }
            self.pos = None;
            let Some(rect) = self.rect() else {
                return true;
            };
            let (started, samples) = self.drag.as_mut().unwrap();
            let t = started.elapsed().as_secs_f64();
            samples.push((t, (rect.x, rect.y)));
            samples.retain(|(st, _)| t - st <= DRAG_SAMPLE_SECS);
            return true;
        }

        let Some((_, samples)) = self.drag.take() else {
            return false;
        };
        self.pos = None;
        let _ = self.rect();
        let vel = physics::release_velocity(&samples);
        if vel.0.hypot(vel.1) >= THROW_MIN_SPEED {
            self.start_free_flight(vel);
        } else {
            self.mode = Mode::Ground;
            self.save_position();
        }
        false
    }

    // ---- 定期的な確認 ----

    /// 画面の抜き差し・全画面アプリ・乗っている窓の様子を確認する
    fn check_windows(&mut self) {
        // ディスプレイの抜き差しや設定の変更で倍率が変わりうるので、ここで読み直す
        if let Some(primary) = self.window.primary_monitor().ok().flatten() {
            self.cursor_scale = primary.scale_factor();
        }
        self.size = None;
        if let Some(pos) = geometry::bring_back_if_lost(&self.window) {
            self.stop_walking();
            self.mode = Mode::Ground;
            self.set_flying(false);
            self.pos = Some(pos);
            self.save_position();
        }
        let Some(rect) = self.rect() else { return };
        let windows = system::on_screen_windows();

        let hide_on_fullscreen = self.shared.store.get().hide_on_fullscreen;
        let covered = hide_on_fullscreen
            && self
                .screen_of(rect)
                .is_some_and(|screen| system::is_fullscreen(&windows, screen.full));
        if covered != self.suppressed {
            self.suppressed = covered;
            self.shared.suppressed.store(covered, Ordering::Relaxed);
            // window.hide()/show() はフォーカスを奪うことがあるため、中身を隠すだけにする
            let _ = self
                .window
                .emit("mascot-suppress", FlagPayload { on: covered });
            if covered {
                self.shared.hovering.store(false, Ordering::Relaxed);
                let _ = self.window.set_ignore_cursor_events(true);
            }
        }

        if let Mode::Perched(perch) = self.mode {
            self.follow_perch(perch, rect, &windows);
        }
    }

    /// 乗っている窓が動いたら一緒に動き、閉じたり小さくなったりしたら落ちる
    fn follow_perch(&mut self, perch: Perch, rect: Rect, windows: &[WindowInfo]) {
        let Some(now) = windows
            .iter()
            .find(|w| w.id == perch.window_id && w.layer == 0)
            .map(|w| w.bounds)
        else {
            self.start_free_flight((0.0, 0.0));
            return;
        };
        let (dx, dy) = (now.x - perch.bounds.x, now.y - perch.bounds.y);
        let feet_x = rect.x + rect.w / 2.0 + dx;
        if feet_x < now.x || feet_x > now.x + now.w {
            self.start_free_flight((0.0, 0.0));
            return;
        }
        if dx != 0.0 || dy != 0.0 {
            self.move_to((rect.x + dx, rect.y + dy));
            if let Some(target) = &mut self.target {
                target.0 += dx;
                target.1 += dy;
            }
        }
        self.mode = Mode::Perched(Perch {
            window_id: perch.window_id,
            bounds: now,
        });
    }

    fn check_sleep_and_break(&mut self) {
        let idle = system::seconds_since_last_input();
        let sleepy = idle >= SLEEP_AFTER_SECS;
        if sleepy != self.sleeping {
            self.sleeping = sleepy;
            let _ = self.window.emit("mascot-sleep", FlagPayload { on: sleepy });
        }

        if idle >= BREAK_RESET_SECS {
            self.working_since = None;
            return;
        }
        let since = *self.working_since.get_or_insert_with(Instant::now);
        if since.elapsed() >= BREAK_REMIND_AFTER {
            self.working_since = Some(Instant::now());
            if self.shared.store.get().chatty {
                let _ = self.window.emit("mascot-say", SayPayload { kind: "break" });
            }
        }
    }

    // ---- 次の行動 ----

    fn decide_next(&mut self) {
        let Some(rect) = self.rect() else { return };
        match self.mode {
            Mode::Perched(perch) => {
                let windows = system::on_screen_windows();
                let spots = match (
                    self.screen_of(rect),
                    windows.iter().position(|w| w.id == perch.window_id),
                ) {
                    (Some(screen), Some(index)) => {
                        perch_spots(&windows, index, rect, &screen, self.stance())
                    }
                    _ => Vec::new(),
                };
                if !spots.is_empty() && self.rng.chance(STAY_ON_PERCH_CHANCE) {
                    // ふちの上の、見えている場所へ歩く
                    let x = spots[(self.rng.next_f64() * spots.len() as f64) as usize];
                    self.target = Some((x, rect.y));
                } else {
                    // ぴょんと飛び降りる
                    let dir = if self.rng.chance(0.5) { -1.0 } else { 1.0 };
                    let vel = (
                        dir * self.rng.range(150.0, 300.0),
                        -self.rng.range(350.0, 550.0),
                    );
                    self.start_free_flight(vel);
                }
            }
            Mode::Ground => {
                if self.rng.chance(PERCH_CHANCE) {
                    let windows = system::on_screen_windows();
                    let stance = self.stance();
                    if let Some(screen) = self.screen_of(rect) {
                        if let Some((to, perch)) =
                            find_perch(&windows, rect, &screen, stance, &mut self.rng)
                        {
                            self.start_jump(to, perch);
                            return;
                        }
                    }
                }
                let screens = geometry::screens(&self.window);
                self.target = pick_ground_target(rect, &screens, &mut self.rng);
            }
            Mode::Flying(_) => {}
        }
    }

    fn walk(&mut self, goal: (f64, f64), dt: f64) {
        let Some(rect) = self.rect() else { return };
        let settings = self.shared.store.get();
        let step = settings.speed.px_per_sec() * settings.scale * dt;
        let (dx, dy) = (goal.0 - rect.x, goal.1 - rect.y);
        let dist = dx.hypot(dy);

        if dist <= step {
            self.move_to(goal);
            self.stop_walking();
            self.save_position();
            self.rest();
            return;
        }
        self.set_moving(true, if dx < 0.0 { -1 } else { 1 });
        self.move_to((rect.x + dx / dist * step, rect.y + dy / dist * step));
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_tick).as_secs_f64().min(0.1);
        self.last_tick = now;

        if self.shared.resync.swap(false, Ordering::Relaxed) {
            self.resync();
        }
        if self.shared.reset.swap(false, Ordering::Relaxed) {
            self.stop_walking();
            self.mode = Mode::Ground;
            self.set_flying(false);
            self.pos = None;
            self.size = None;
        }

        let hovering = self.shared.hovering.load(Ordering::Relaxed);
        if self.track_drag(hovering) {
            return;
        }
        if hovering {
            // カーソルが乗っている間は動かされたかもしれないので、覚えている位置は使わない
            self.pos = None;
        }

        let interval = if matches!(self.mode, Mode::Perched(_)) {
            WINDOW_CHECK_PERCHED
        } else {
            WINDOW_CHECK
        };
        if self
            .last_window_check
            .is_none_or(|t| t.elapsed() >= interval)
        {
            self.last_window_check = Some(now);
            self.check_windows();
        }
        if self
            .last_sleep_check
            .is_none_or(|t| t.elapsed() >= SLEEP_CHECK)
        {
            self.last_sleep_check = Some(now);
            self.check_sleep_and_break();
        }

        if matches!(self.mode, Mode::Flying(_)) {
            self.flight_step(dt);
            return;
        }

        let settings = self.shared.store.get();
        let paused = hovering
            || self.sleeping
            || self.suppressed
            || !settings.walking
            || !self.shared.visible.load(Ordering::Relaxed);
        if paused {
            self.stop_walking();
            self.idle_until = now + Duration::from_millis(IDLE_MIN_MS);
            return;
        }

        match self.target {
            Some(goal) => self.walk(goal, dt),
            None if now >= self.idle_until => self.decide_next(),
            None => {}
        }
    }
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>) {
    thread::spawn(move || {
        let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
            return;
        };
        let mut brain = Brain::new(window, shared);
        loop {
            thread::sleep(brain.interval());
            brain.tick();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: f64, y: f64, w: f64, h: f64) -> Screen {
        Screen {
            full: Rect::new(x, y, w, h),
            work: Rect::new(x, y + 39.0, w, h - 39.0 - 80.0),
        }
    }

    fn app_window(id: i64, bounds: Rect) -> WindowInfo {
        WindowInfo {
            id,
            layer: 0,
            bounds,
            own: false,
        }
    }

    const MASCOT: Rect = Rect {
        x: 100.0,
        y: 600.0,
        w: 148.0,
        h: 206.0,
    };

    #[test]
    fn ground_target_stays_in_work_area() {
        let screens = [screen(0.0, 0.0, 1800.0, 1169.0)];
        let mut rng = Rng(12345);
        for _ in 0..1000 {
            let (x, y) = pick_ground_target(MASCOT, &screens, &mut rng).unwrap();
            let work = screens[0].work;
            assert!(x >= work.x && x + MASCOT.w <= work.x + work.w);
            assert!(y >= work.y && y + MASCOT.h <= work.y + work.h);
        }
    }

    #[test]
    fn ground_target_sometimes_goes_to_other_screen() {
        let screens = [
            screen(0.0, 0.0, 1800.0, 1169.0),
            screen(1800.0, 0.0, 1920.0, 1080.0),
        ];
        let mut rng = Rng(987);
        let others = (0..1000)
            .filter(|_| pick_ground_target(MASCOT, &screens, &mut rng).unwrap().0 >= 1800.0)
            .count();
        assert!(others > 150 && others < 350, "{others}");
    }

    const STANCE: Stance = Stance {
        foot_offset: 21.0,
        head_height: 105.0,
    };

    #[test]
    fn perch_puts_feet_on_window_edge() {
        let s = screen(0.0, 0.0, 1800.0, 1169.0);
        let windows = [app_window(7, Rect::new(400.0, 500.0, 800.0, 400.0))];
        let mut rng = Rng(42);
        let ((x, y), perch) = find_perch(&windows, MASCOT, &s, STANCE, &mut rng).unwrap();
        assert_eq!(perch.window_id, 7);
        assert_eq!(y + MASCOT.h - 21.0, 500.0);
        assert!(x >= 400.0 && x + MASCOT.w <= 1200.0);
    }

    #[test]
    fn no_perch_on_narrow_or_too_high_windows() {
        let s = screen(0.0, 0.0, 1800.0, 1169.0);
        let mut rng = Rng(42);
        let narrow = [app_window(1, Rect::new(400.0, 500.0, 150.0, 400.0))];
        assert!(find_perch(&narrow, MASCOT, &s, STANCE, &mut rng).is_none());
        // ふちのすぐ上に頭が入る余白がない（メニューバーに頭がめりこむ）
        let too_high = [app_window(1, Rect::new(400.0, 120.0, 800.0, 400.0))];
        assert!(find_perch(&too_high, MASCOT, &s, STANCE, &mut rng).is_none());
    }

    #[test]
    fn perch_only_on_visible_part_of_edge() {
        let s = screen(0.0, 0.0, 1800.0, 1169.0);
        // 奥の窓（x 0〜1200）の右側を、手前の窓（x 300〜）が隠している
        let windows = [
            app_window(1, Rect::new(300.0, 100.0, 1500.0, 1000.0)),
            app_window(2, Rect::new(0.0, 400.0, 1200.0, 500.0)),
        ];
        let spots = perch_spots(&windows, 1, MASCOT, &s, STANCE);
        assert!(!spots.is_empty());
        for x in spots {
            assert!(x + MASCOT.w / 2.0 < 300.0, "{x}");
        }
    }

    #[test]
    fn rng_range_is_within_bounds() {
        let mut rng = Rng::new();
        for _ in 0..10_000 {
            let v = rng.range(2.0, 5.0);
            assert!((2.0..5.0).contains(&v));
        }
    }
}
