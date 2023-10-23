use std::time::Duration;

use serde::{Deserialize, Serialize};

use math::math::{mix, vector::vec2};

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
pub struct TeeAnimationFrame {
    pub pos: vec2,
    pub scale: vec2,
    pub rotation: f32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TeeAnimationFrames {
    // key = timestamp in nanoseconds
    pub frames: Vec<(Duration, TeeAnimationFrame)>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TeeAnimation {
    pub body: TeeAnimationFrames,
    pub left_eye: TeeAnimationFrames,
    pub right_eye: TeeAnimationFrames,
    pub left_foot: TeeAnimationFrames,
    pub right_foot: TeeAnimationFrames,
    pub left_hand: TeeAnimationFrames,
    pub right_hand: TeeAnimationFrames,
}

#[derive(Copy, Clone, Default)]
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
        if frames.frames.len() == 0 {
            frame.pos = Default::default();
            frame.scale = Default::default();
            frame.rotation = Default::default();
        } else if frames.frames.len() == 1 {
            *frame = frames.frames[0].1;
        } else {
            //time = maximum(0.0f, minimum(1.0f, time / duration)); // TODO: use clamp
            let mut frame1: Option<&TeeAnimationFrame> = None;
            let mut frame2: Option<&TeeAnimationFrame> = None;
            let mut blend = 0.0;

            // TODO: make this smarter.. binary search
            for i in 1..frames.frames.len() {
                if frames.frames[i - 1].0 <= *time && frames.frames[i].0 >= *time {
                    frame1 = Some(&frames.frames[i - 1].1);
                    frame2 = Some(&frames.frames[i].1);
                    blend = (*time - frames.frames[i - 1].0).as_secs_f32()
                        / (frames.frames[i].0 - frames.frames[i - 1].0).as_secs_f32();
                    break;
                }
            }

            if frame1.is_some() && frame2.is_some() {
                frame.pos.x = mix(&frame1.unwrap().pos.x, &frame2.unwrap().pos.x, blend);
                frame.pos.y = mix(&frame1.unwrap().pos.y, &frame2.unwrap().pos.y, blend);
                frame.rotation = mix(&frame1.unwrap().rotation, &frame2.unwrap().rotation, blend);
                frame.scale.x = mix(&frame1.unwrap().scale.x, &frame2.unwrap().scale.x, blend);
                frame.scale.y = mix(&frame1.unwrap().scale.y, &frame2.unwrap().scale.y, blend);
            }
        }
    }

    fn anim_add_keyframe(dest: &mut TeeAnimationFrame, added: &TeeAnimationFrame, amount: f32) {
        // AnimSeqEval fills m_X for any case, clang-analyzer assumes going into the
        // final else branch with frames.m_NumFrames < 2, which is impossible.
        dest.pos.x += added.pos.x * amount;
        dest.pos.y += added.pos.y * amount;
        dest.scale.y += added.scale.y * amount;
        dest.scale.y += added.scale.y * amount;
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
