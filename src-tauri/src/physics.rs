//! 投げる・落ちる・飛び乗る、の動きの計算。座標は macOS のポイント単位、y は下向き

/// 重力加速度（pt/秒²）
pub const GRAVITY: f64 = 2400.0;
/// 壁・天井で跳ね返るときに残る速さの割合
const WALL_BOUNCE: f64 = 0.55;
/// 床で跳ね返るときに残る速さの割合
const FLOOR_BOUNCE: f64 = 0.45;
/// 床で跳ねるたびに横の速さに掛ける摩擦
const FLOOR_FRICTION: f64 = 0.7;
/// 床に当たったときの縦の速さがこれ未満なら着地する（pt/秒）
const LAND_SPEED: f64 = 220.0;
/// 投げたときの速さの上限（pt/秒）
const MAX_SPEED: f64 = 3000.0;

/// ウィンドウ左上が動ける範囲
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub pos: (f64, f64),
    pub vel: (f64, f64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Flying,
    Landed,
}

/// dt 秒だけ動かす。範囲の端では跳ね返り、床で勢いがなくなったら着地する
pub fn step(body: &mut Body, bounds: &Bounds, dt: f64) -> Step {
    body.vel.1 += GRAVITY * dt;
    body.pos.0 += body.vel.0 * dt;
    body.pos.1 += body.vel.1 * dt;

    if body.pos.0 < bounds.min_x {
        body.pos.0 = bounds.min_x;
        body.vel.0 = body.vel.0.abs() * WALL_BOUNCE;
    } else if body.pos.0 > bounds.max_x {
        body.pos.0 = bounds.max_x;
        body.vel.0 = -body.vel.0.abs() * WALL_BOUNCE;
    }
    if body.pos.1 < bounds.min_y {
        body.pos.1 = bounds.min_y;
        body.vel.1 = body.vel.1.abs() * WALL_BOUNCE;
    }
    if body.pos.1 >= bounds.max_y {
        body.pos.1 = bounds.max_y;
        if body.vel.1 < LAND_SPEED {
            body.vel = (0.0, 0.0);
            return Step::Landed;
        }
        body.vel.1 = -body.vel.1 * FLOOR_BOUNCE;
        body.vel.0 *= FLOOR_FRICTION;
    }
    Step::Flying
}

/// from から to へ、放物線を描いて duration 秒でちょうど着くための初速
pub fn aim(from: (f64, f64), to: (f64, f64), duration: f64) -> (f64, f64) {
    (
        (to.0 - from.0) / duration,
        (to.1 - from.1 - 0.5 * GRAVITY * duration * duration) / duration,
    )
}

/// 初速 vel で飛び出してから t 秒後の位置
pub fn position_at(from: (f64, f64), vel: (f64, f64), t: f64) -> (f64, f64) {
    (
        from.0 + vel.0 * t,
        from.1 + vel.1 * t + 0.5 * GRAVITY * t * t,
    )
}

/// ドラッグ中に記録した (経過秒, 位置) から、手を離した瞬間の速度を求める
pub fn release_velocity(samples: &[(f64, (f64, f64))]) -> (f64, f64) {
    let (Some(first), Some(last)) = (samples.first(), samples.last()) else {
        return (0.0, 0.0);
    };
    let dt = last.0 - first.0;
    if dt < 0.02 {
        return (0.0, 0.0);
    }
    let vel = ((last.1 .0 - first.1 .0) / dt, (last.1 .1 - first.1 .1) / dt);
    let speed = vel.0.hypot(vel.1);
    if speed > MAX_SPEED {
        (vel.0 / speed * MAX_SPEED, vel.1 / speed * MAX_SPEED)
    } else {
        vel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Bounds = Bounds {
        min_x: 0.0,
        max_x: 1000.0,
        min_y: 0.0,
        max_y: 800.0,
    };

    fn simulate(mut body: Body) -> (Body, usize) {
        for i in 0..10_000 {
            if step(&mut body, &BOUNDS, 1.0 / 60.0) == Step::Landed {
                return (body, i);
            }
        }
        panic!("着地しなかった");
    }

    #[test]
    fn dropped_body_lands_on_the_floor() {
        let (body, _) = simulate(Body {
            pos: (500.0, 100.0),
            vel: (0.0, 0.0),
        });
        assert_eq!(body.pos.1, BOUNDS.max_y);
        assert_eq!(body.vel, (0.0, 0.0));
    }

    #[test]
    fn thrown_body_stays_inside_bounds() {
        let mut body = Body {
            pos: (500.0, 400.0),
            vel: (2500.0, -2500.0),
        };
        for _ in 0..600 {
            step(&mut body, &BOUNDS, 1.0 / 60.0);
            assert!(body.pos.0 >= BOUNDS.min_x && body.pos.0 <= BOUNDS.max_x);
            assert!(body.pos.1 >= BOUNDS.min_y && body.pos.1 <= BOUNDS.max_y);
        }
    }

    #[test]
    fn bounces_off_walls() {
        let mut body = Body {
            pos: (990.0, 400.0),
            vel: (1000.0, 0.0),
        };
        step(&mut body, &BOUNDS, 1.0 / 60.0);
        assert!(body.vel.0 < 0.0);
    }

    #[test]
    fn aim_reaches_target_on_time() {
        let (from, to, t) = ((100.0, 700.0), (600.0, 300.0), 0.8);
        let vel = aim(from, to, t);
        let end = position_at(from, vel, t);
        assert!((end.0 - to.0).abs() < 1e-9);
        assert!((end.1 - to.1).abs() < 1e-9);
    }

    #[test]
    fn release_velocity_from_samples() {
        let samples = [
            (0.0, (0.0, 0.0)),
            (0.05, (50.0, -25.0)),
            (0.1, (100.0, -50.0)),
        ];
        let (vx, vy) = release_velocity(&samples);
        assert!((vx - 1000.0).abs() < 1e-9);
        assert!((vy + 500.0).abs() < 1e-9);
    }

    #[test]
    fn release_velocity_is_capped_and_ignores_short_samples() {
        assert_eq!(release_velocity(&[]), (0.0, 0.0));
        assert_eq!(
            release_velocity(&[(0.0, (0.0, 0.0)), (0.01, (100.0, 0.0))]),
            (0.0, 0.0)
        );
        let (vx, vy) = release_velocity(&[(0.0, (0.0, 0.0)), (0.1, (10_000.0, 0.0))]);
        assert!((vx.hypot(vy) - MAX_SPEED).abs() < 1e-9);
    }
}
