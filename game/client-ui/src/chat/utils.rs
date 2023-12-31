use std::{
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use client_containers::skins::{SkinContainer, TeeSkinEye};
use client_render_base::render::{
    animation::AnimState,
    default_anim::{base_anim, idle_anim},
    tee::{RenderTee, TeeRenderHands, TeeRenderInfo, TeeRenderSkinTextures},
};
use egui::{Pos2, Rect, Vec2};
use graphics::{graphics::Graphics, streaming::DrawScopeImpl};
use graphics_types::{
    rendering::{ColorRGBA, State},
    types::StreamedQuad,
};
use math::math::vector::{vec2, vec4};
use ui_base::custom_callback::{CustomCallback, CustomCallbackTrait};

pub fn render_tee_for_chat(
    ui: &mut egui::Ui,
    render_rect: Rect,
    clip_rect: Option<Rect>,
    pos: vec2,
    size: f32,
) {
    struct RenderTeeCB {
        render_rect: Rect,
        clip_rect: Option<Rect>,
        pos: vec2,
        size: f32,
    }
    impl CustomCallbackTrait<SkinContainer, RenderTee, ()> for RenderTeeCB {
        fn render2(
            &self,
            graphics: &mut Graphics,
            callback_custom_type1: &mut SkinContainer,
            callback_custom_type2: &mut RenderTee,
        ) {
            let mut anim_state = AnimState::default();
            anim_state.set(&base_anim(), &Duration::from_millis(0));
            anim_state.add(&idle_anim(), &Duration::from_millis(0), 1.0);

            let skin = callback_custom_type1.get_or_default("TODO:");
            let tee_render_info = TeeRenderInfo {
                render_skin: TeeRenderSkinTextures::Colorable(&skin.grey_scaled_textures),
                color_body: ColorRGBA {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                color_feet: ColorRGBA {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                metrics: &skin.metrics,
                got_air_jump: false,
                feet_flipped: false,
                size: self.size,
            };

            let dir = vec2::new(1.0, 0.0);

            let mut state = State::new();
            state.map_canvas(
                self.render_rect.min.x,
                self.render_rect.min.y,
                self.render_rect.max.x,
                self.render_rect.max.y,
            );
            let ppp = graphics.canvas_handle.window_pixels_per_point();
            if let Some(clip_rect) = &self.clip_rect {
                state.clip_auto_rounding(
                    clip_rect.min.x * ppp,
                    clip_rect.min.y * ppp,
                    clip_rect.width() * ppp,
                    clip_rect.height() * ppp,
                );
            }

            callback_custom_type2.render_tee(
                &anim_state,
                &tee_render_info,
                TeeSkinEye::Normal,
                &TeeRenderHands {
                    left: None,
                    right: None,
                },
                &dir,
                &self.pos,
                1.0,
                &state,
            );
        }
    }

    let cb = RenderTeeCB {
        render_rect,
        clip_rect,
        pos,
        size,
    };

    let custom = egui::PaintCallback {
        callback: Arc::new(CustomCallback::<SkinContainer, RenderTee, ()>::new(
            Box::new(cb),
            2,
        )),
        rect: render_rect,
    };
    ui.painter().add(custom);
}

// floats times 1000000000
#[derive(Debug, Default)]
pub struct RenderRectAfterwards {
    pub x: AtomicU64,
    pub y: AtomicU64,
    pub w: AtomicU64,
    pub h: AtomicU64,
}

/// little hack to render a rect behind later rendered ui elements
pub fn render_rect_afterwards(
    ui: &mut egui::Ui,
    render_rect: Rect,
    clip_rect: Option<Rect>,
    color: vec4,
) -> Arc<RenderRectAfterwards> {
    let rect = Arc::new(RenderRectAfterwards::default());
    struct RenderRectCB {
        rect: Arc<RenderRectAfterwards>,
        render_rect: Rect,
        clip_rect: Option<Rect>,
        color: vec4,
    }
    impl CustomCallbackTrait<SkinContainer, RenderTee, ()> for RenderRectCB {
        fn render2(
            &self,
            graphics: &mut Graphics,
            _callback_custom_type1: &mut SkinContainer,
            _callback_custom_type2: &mut RenderTee,
        ) {
            let rect = Rect::from_min_size(
                Pos2::new(
                    (self.rect.x.load(std::sync::atomic::Ordering::SeqCst) as f64 / 1000000000.0)
                        as f32,
                    (self.rect.y.load(std::sync::atomic::Ordering::SeqCst) as f64 / 1000000000.0)
                        as f32,
                ),
                Vec2::new(
                    (self.rect.w.load(std::sync::atomic::Ordering::SeqCst) as f64 / 1000000000.0)
                        as f32,
                    (self.rect.h.load(std::sync::atomic::Ordering::SeqCst) as f64 / 1000000000.0)
                        as f32,
                ),
            );

            let mut state = State::new();
            state.map_canvas(
                self.render_rect.min.x,
                self.render_rect.min.y,
                self.render_rect.max.x,
                self.render_rect.max.y,
            );
            let ppp = graphics.canvas_handle.window_pixels_per_point();
            let mut quads = graphics.stream_handle.quads_begin();
            quads.set_state(&state);
            quads.set_colors_from_single(
                self.color.r(),
                self.color.g(),
                self.color.b(),
                self.color.a(),
            );
            if let Some(clip_rect) = &self.clip_rect {
                quads.clip_auto_rounding(
                    clip_rect.min.x * ppp,
                    clip_rect.min.y * ppp,
                    clip_rect.width() * ppp,
                    clip_rect.height() * ppp,
                );
            }
            quads.quads_draw_tl(&[StreamedQuad::from_pos_and_size(
                rect.min.x,
                rect.min.y,
                rect.width(),
                rect.height(),
            )]);
        }
    }

    let cb = RenderRectCB {
        rect: rect.clone(),
        render_rect,
        clip_rect,
        color,
    };

    let custom = egui::PaintCallback {
        callback: Arc::new(CustomCallback::<SkinContainer, RenderTee, ()>::new(
            Box::new(cb),
            2,
        )),
        rect: Rect::NOTHING,
    };
    ui.painter().add(custom);
    rect
}
