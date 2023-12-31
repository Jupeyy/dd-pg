use std::time::Duration;

use math::math::vector::vec2;

use super::animation::{TeeAnimation, TeeAnimationFrame, TeeAnimationFrames};

pub fn base_anim() -> TeeAnimation {
    let mut anim = TeeAnimation::default();
    anim.body.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, -4.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, 10.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, 10.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_eye.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_eye.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn idle_anim() -> TeeAnimation {
    let mut anim = TeeAnimation::default();
    anim.left_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(-7.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(7.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn inair_anim() -> TeeAnimation {
    let mut anim = TeeAnimation::default();
    anim.left_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(-3.0, 0.0),
            rotation: -0.1,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(3.0, 0.0),
            rotation: -0.1,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn sit_left_anim() -> TeeAnimation {
    let mut anim = TeeAnimation::default();
    anim.body.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, 3.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(-12.0, 0.0),
            rotation: -0.1,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(-8.0, 0.0),
            rotation: -0.1,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn sit_right_anim() -> TeeAnimation {
    let mut anim = TeeAnimation::default();
    anim.body.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, 3.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(12.0, 0.0),
            rotation: -0.1,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(8.0, 0.0),
            rotation: -0.1,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn walk_anim() -> TeeAnimation {
    let mut anim = TeeAnimation::default();
    // body
    anim.body.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(200),
        TeeAnimationFrame {
            pos: vec2::new(0.0, -1.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(400),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(600),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(800),
        TeeAnimationFrame {
            pos: vec2::new(0.0, -1.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));

    // left foot
    anim.left_foot.frames.push((
        Duration::from_millis(0),
        TeeAnimationFrame {
            pos: vec2::new(8.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(200),
        TeeAnimationFrame {
            pos: vec2::new(-8.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(400),
        TeeAnimationFrame {
            pos: vec2::new(-10.0, -4.0),
            rotation: 0.2,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(600),
        TeeAnimationFrame {
            pos: vec2::new(-8.0, -8.0),
            rotation: 0.3,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(800),
        TeeAnimationFrame {
            pos: vec2::new(4.0, -4.0),
            rotation: -0.2,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(8.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));

    // right foot
    anim.right_foot.frames.push((
        Duration::from_millis(0),
        TeeAnimationFrame {
            pos: vec2::new(-10.0, -4.0),
            rotation: 0.2,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(200),
        TeeAnimationFrame {
            pos: vec2::new(-8.0, -8.0),
            rotation: 0.3,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(400),
        TeeAnimationFrame {
            pos: vec2::new(4.0, -4.0),
            rotation: -0.2,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(600),
        TeeAnimationFrame {
            pos: vec2::new(8.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(800),
        TeeAnimationFrame {
            pos: vec2::new(8.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(-10.0, -4.0),
            rotation: 0.2,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn run_left_anim() -> TeeAnimation {
    let mut anim = TeeAnimation::default();
    // body
    anim.body.frames.push((
        Duration::ZERO,
        TeeAnimationFrame {
            pos: vec2::new(0.0, -1.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(200),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(400),
        TeeAnimationFrame {
            pos: vec2::new(0.0, -1.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(600),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(800),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.body.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(0.0, -1.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));

    // left foot
    anim.left_foot.frames.push((
        Duration::from_millis(0),
        TeeAnimationFrame {
            pos: vec2::new(18.0, -8.0),
            rotation: -0.27,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(200),
        TeeAnimationFrame {
            pos: vec2::new(6.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(400),
        TeeAnimationFrame {
            pos: vec2::new(-7.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(600),
        TeeAnimationFrame {
            pos: vec2::new(-13.0, -4.5),
            rotation: 0.05,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(800),
        TeeAnimationFrame {
            pos: vec2::new(0.0, -8.0),
            rotation: -0.2,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.left_foot.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(18.0, -8.0),
            rotation: -0.27,
            scale: vec2::new(1.0, 1.0),
        },
    ));

    // right foot
    anim.right_foot.frames.push((
        Duration::from_millis(0),
        TeeAnimationFrame {
            pos: vec2::new(-11.0, -2.5),
            rotation: 0.05,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(200),
        TeeAnimationFrame {
            pos: vec2::new(-14.0, -5.0),
            rotation: 0.1,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(400),
        TeeAnimationFrame {
            pos: vec2::new(11.0, -8.0),
            rotation: -0.3,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(600),
        TeeAnimationFrame {
            pos: vec2::new(18.0, -8.0),
            rotation: -0.27,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(800),
        TeeAnimationFrame {
            pos: vec2::new(3.0, 0.0),
            rotation: 0.0,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.right_foot.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(-11.0, -2.5),
            rotation: 0.05,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn run_right_anim() -> TeeAnimation {
    let mut anim = run_left_anim();
    anim.body
        .frames
        .iter_mut()
        .for_each(|f| f.1.pos.x = -f.1.pos.x);
    anim.left_foot
        .frames
        .iter_mut()
        .for_each(|f| f.1.pos.x = -f.1.pos.x);
    anim.right_foot
        .frames
        .iter_mut()
        .for_each(|f| f.1.pos.x = -f.1.pos.x);
    anim
}

pub fn hammer_swing_anim() -> TeeAnimationFrames {
    let mut anim = TeeAnimationFrames::default();
    anim.frames.push((
        Duration::from_millis(0),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: -0.10,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(300),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.25,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(400),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.30,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(500),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.25,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: -0.10,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}

pub fn ninja_swing_anim() -> TeeAnimationFrames {
    let mut anim = TeeAnimationFrames::default();
    anim.frames.push((
        Duration::from_millis(0),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: -0.25,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(100),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: -0.05,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(150),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.35,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(420),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.40,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(500),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: 0.35,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim.frames.push((
        Duration::from_millis(1000),
        TeeAnimationFrame {
            pos: vec2::new(0.0, 0.0),
            rotation: -0.25,
            scale: vec2::new(1.0, 1.0),
        },
    ));
    anim
}
