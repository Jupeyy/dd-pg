use std::time::Duration;

use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

use math::math::{lerp, mix, vector::vec2};

#[derive(Debug, Hiarc, Copy, Clone, Default, Serialize, Deserialize)]
pub struct TeeAnimationFrame {
    pub pos: vec2,
    pub scale: vec2,
    pub rotation: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeeAnimationFrames {
    // key = timestamp in nanoseconds
    pub frames: Vec<(Duration, TeeAnimationFrame)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeeAnimation {
    pub body: TeeAnimationFrames,
    pub left_eye: TeeAnimationFrames,
    pub right_eye: TeeAnimationFrames,
    pub left_foot: TeeAnimationFrames,
    pub right_foot: TeeAnimationFrames,
    pub left_hand: TeeAnimationFrames,
    pub right_hand: TeeAnimationFrames,
}

#[derive(Debug, Hiarc, Copy, Clone, Default)]
pub struct AnimState {
    pub body: TeeAnimationFrame,
    pub left_eye: TeeAnimationFrame,
    pub right_eye: TeeAnimationFrame,
    pub left_foot: TeeAnimationFrame,
    pub right_foot: TeeAnimationFrame,
    pub left_hand: TeeAnimationFrame,
    pub right_hand: TeeAnimationFrame,
}

impl AnimState {
    pub fn anim_frame_eval(
        frames: &TeeAnimationFrames,
        time: &Duration,
        frame: &mut TeeAnimationFrame,
    ) {
        if frames.frames.is_empty() {
            frame.pos = Default::default();
            frame.scale = vec2::new(1.0, 1.0);
            frame.rotation = Default::default();
        } else if frames.frames.len() == 1 {
            *frame = frames.frames[0].1;
        } else {
            let time = time
                .as_secs_f32()
                .rem_euclid(frames.frames.last().unwrap().0.as_secs_f32());

            let i = frames
                .frames
                .partition_point(|(a, _)| a.as_secs_f32() <= time);
            let i_prev = i.saturating_sub(1);
            let i = i.clamp(0, frames.frames.len() - 1);
            let frame1 = &frames.frames[i_prev].1;
            let frame2 = &frames.frames[i].1;
            let dividend = time - frames.frames[i_prev].0.as_secs_f32();
            let divisor = (frames.frames[i].0 - frames.frames[i_prev].0).as_secs_f32();
            let blend = if divisor > 0.0 {
                dividend / divisor
            } else {
                0.0
            };

            frame.pos.x = mix(&frame1.pos.x, &frame2.pos.x, blend);
            frame.pos.y = mix(&frame1.pos.y, &frame2.pos.y, blend);
            frame.rotation = mix(&frame1.rotation, &frame2.rotation, blend);
            frame.scale.x = mix(&frame1.scale.x, &frame2.scale.x, blend);
            frame.scale.y = mix(&frame1.scale.y, &frame2.scale.y, blend);
        }
    }

    fn anim_add_keyframe(dest: &mut TeeAnimationFrame, added: &TeeAnimationFrame, amount: f32) {
        dest.pos.x += added.pos.x * amount;
        dest.pos.y += added.pos.y * amount;
        dest.scale.x = lerp(&dest.scale.x, &added.scale.x, amount);
        dest.scale.y = lerp(&dest.scale.y, &added.scale.y, amount);
        dest.rotation += added.rotation * amount;
    }

    fn anim_add(&mut self, added: &AnimState, amount: f32) {
        Self::anim_add_keyframe(&mut self.body, &added.body, amount);
        Self::anim_add_keyframe(&mut self.left_eye, &added.left_eye, amount);
        Self::anim_add_keyframe(&mut self.right_eye, &added.right_eye, amount);
        Self::anim_add_keyframe(&mut self.left_foot, &added.left_foot, amount);
        Self::anim_add_keyframe(&mut self.right_foot, &added.right_foot, amount);
        Self::anim_add_keyframe(&mut self.left_hand, &added.left_hand, amount);
        Self::anim_add_keyframe(&mut self.right_hand, &added.right_hand, amount);
    }

    pub fn set(&mut self, anim: &TeeAnimation, time: &Duration) {
        Self::anim_frame_eval(&anim.body, time, &mut self.body);
        Self::anim_frame_eval(&anim.left_eye, time, &mut self.left_eye);
        Self::anim_frame_eval(&anim.right_eye, time, &mut self.right_eye);
        Self::anim_frame_eval(&anim.left_foot, time, &mut self.left_foot);
        Self::anim_frame_eval(&anim.right_foot, time, &mut self.right_foot);
        Self::anim_frame_eval(&anim.left_hand, time, &mut self.left_hand);
        Self::anim_frame_eval(&anim.right_hand, time, &mut self.right_hand);
    }

    pub fn add(&mut self, anim: &TeeAnimation, time: &Duration, amount: f32) {
        let mut add_state = Self::default();
        add_state.set(anim, time);
        self.anim_add(&add_state, amount);
    }
}
