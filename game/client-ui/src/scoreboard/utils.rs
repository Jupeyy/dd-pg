use std::{sync::Arc, time::Duration};

use client_containers::skins::{SkinContainer, TeeSkinEye};
use client_render_base::render::{
    animation::AnimState,
    default_anim::{base_anim, idle_anim},
    tee::{RenderTee, TeeRenderHands, TeeRenderInfo, TeeRenderSkinTextures},
};
use egui::Rect;
use graphics::graphics::Graphics;
use graphics_types::rendering::{ColorRGBA, State};
use math::math::vector::vec2;
use ui_base::custom_callback::{CustomCallback, CustomCallbackTrait};

pub fn render_tee_for_scoreboard(
    ui: &mut egui::Ui,
    render_rect: Rect,
    clip_rect: Option<Rect>,
    skin_name: &str,
    pos: vec2,
    size: f32,
) {
    struct RenderTeeCB {
        render_rect: Rect,
        clip_rect: Option<Rect>,
        skin_name: String,
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

            let skin = callback_custom_type1.get_or_default(&self.skin_name);
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
        skin_name: skin_name.to_string(),
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
