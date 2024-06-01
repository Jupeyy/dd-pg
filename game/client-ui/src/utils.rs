use std::{rc::Rc, time::Duration};

use client_containers_new::skins::{Skin, SkinContainer};
use client_render_base::render::{
    animation::AnimState,
    default_anim::{base_anim, idle_anim},
    tee::{offset_to_mid, RenderTee, TeeRenderHands, TeeRenderInfo, TeeRenderSkinTextures},
};
use egui::Rect;
use game_interface::{types::render::character::TeeEye, types::resource_key::ResourceKey};
use graphics::handles::canvas::canvas::GraphicsCanvasHandle;
use graphics_types::rendering::{ColorRGBA, State};
use math::math::vector::vec2;
use ui_base::{custom_callback::CustomCallbackTrait, types::UiState};

pub fn render_tee_for_ui(
    canvas_handle: &GraphicsCanvasHandle,
    skin_container: &mut SkinContainer,
    render_tee: &RenderTee,
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    render_rect: Rect,
    clip_rect: Option<Rect>,
    skin: &ResourceKey,
    pos: vec2,
    size: f32,
) {
    #[derive(Debug)]
    struct RenderTeeCB {
        render_rect: Rect,
        clip_rect: Option<Rect>,
        skin: Rc<Skin>,
        pos: vec2,
        size: f32,
        canvas_handle: GraphicsCanvasHandle,
        render_tee: RenderTee,
    }
    impl CustomCallbackTrait for RenderTeeCB {
        fn render(&self) {
            let mut anim_state = AnimState::default();
            anim_state.set(&base_anim(), &Duration::from_millis(0));
            anim_state.add(&idle_anim(), &Duration::from_millis(0), 1.0);

            let tee_render_info = TeeRenderInfo {
                render_skin: TeeRenderSkinTextures::Colorable(&self.skin.grey_scaled_textures),
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
                metrics: &self.skin.metrics,
                got_air_jump: false,
                feet_flipped: false,
                size: self.size,
                eye_left: TeeEye::Normal,
                eye_right: TeeEye::Normal,
            };

            let dir = vec2::new(1.0, 0.0);

            let mut state = State::new();
            state.map_canvas(
                self.render_rect.min.x,
                self.render_rect.min.y,
                self.render_rect.max.x,
                self.render_rect.max.y,
            );
            let scale_x = self.canvas_handle.window_width() as f32 / self.render_rect.width();
            let scale_y = self.canvas_handle.window_height() as f32 / self.render_rect.height();
            if let Some(clip_rect) = &self.clip_rect {
                state.clip_auto_rounding(
                    clip_rect.min.x * scale_x,
                    clip_rect.min.y * scale_y,
                    clip_rect.width() * scale_x,
                    clip_rect.height() * scale_y,
                );
            }

            self.render_tee.render_tee(
                &anim_state,
                &tee_render_info,
                &TeeRenderHands {
                    left: None,
                    right: None,
                },
                &dir,
                &(self.pos + offset_to_mid(&self.skin.metrics, &anim_state, &tee_render_info)),
                1.0,
                &state,
            );
        }
    }

    let skin = skin_container.get_or_default(&skin);
    let cb = RenderTeeCB {
        render_rect,
        clip_rect,
        skin: skin.clone(),
        pos,
        size,
        canvas_handle: canvas_handle.clone(),
        render_tee: render_tee.clone(),
    };

    ui_state.add_custom_paint(ui, render_rect, Rc::new(cb));
}
