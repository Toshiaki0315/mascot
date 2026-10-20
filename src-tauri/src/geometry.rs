//! 位置計算はすべて macOS のポイント（論理ピクセル）単位で行う。
//! 物理ピクセルは画面ごとに倍率が違い、マルチモニターで座標がずれるため。

use tauri::{LogicalPosition, LogicalSize, Monitor, WebviewWindow};

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains(&self, (px, py): (f64, f64)) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    /// 1pt 程度の誤差を許して、o 全体を覆っているか
    pub fn covers(&self, o: &Rect) -> bool {
        const EPS: f64 = 1.0;
        self.x <= o.x + EPS
            && self.y <= o.y + EPS
            && self.x + self.w >= o.x + o.w - EPS
            && self.y + self.h >= o.y + o.h - EPS
    }
}

pub struct Screen {
    /// 画面全体
    pub full: Rect,
    /// メニューバーと Dock を除いた領域
    pub work: Rect,
}

fn screen_of(m: &Monitor) -> Screen {
    let s = m.scale_factor();
    let (pos, size, work) = (m.position(), m.size(), m.work_area());
    Screen {
        full: Rect::new(
            pos.x as f64 / s,
            pos.y as f64 / s,
            size.width as f64 / s,
            size.height as f64 / s,
        ),
        work: Rect::new(
            work.position.x as f64 / s,
            work.position.y as f64 / s,
            work.size.width as f64 / s,
            work.size.height as f64 / s,
        ),
    }
}

pub fn screens(window: &WebviewWindow) -> Vec<Screen> {
    window
        .available_monitors()
        .map(|ms| ms.iter().map(screen_of).collect())
        .unwrap_or_default()
}

pub fn primary_screen(window: &WebviewWindow) -> Option<Screen> {
    window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| screen_of(&m))
}

pub fn window_rect(window: &WebviewWindow) -> Option<Rect> {
    let s = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    Some(Rect::new(
        pos.x as f64 / s,
        pos.y as f64 / s,
        size.width as f64 / s,
        size.height as f64 / s,
    ))
}

pub fn move_window(window: &WebviewWindow, x: f64, y: f64) {
    let _ = window.set_position(LogicalPosition::new(x, y));
}

pub fn resize_window(window: &WebviewWindow, w: f64, h: f64) {
    let _ = window.set_size(LogicalSize::new(w, h));
}

/// 指定した位置にウィンドウを置いたとき、中心がどこかの画面に入っているか
pub fn is_on_screen(window: &WebviewWindow, rect: Rect) -> bool {
    screens(window)
        .iter()
        .any(|s| s.full.contains(rect.center()))
}

/// 中心がどの画面にも入っていなければ（モニターを外した時など）、
/// メイン画面の右下へ戻す。戻した場合はその位置を返す
pub fn bring_back_if_lost(window: &WebviewWindow) -> Option<(f64, f64)> {
    let rect = window_rect(window)?;
    if is_on_screen(window, rect) {
        return None;
    }
    let work = primary_screen(window)?.work;
    let x = work.x + (work.w - rect.w - 40.0).max(0.0);
    let y = work.y + (work.h - rect.h).max(0.0);
    move_window(window, x, y);
    Some((x, y))
}
