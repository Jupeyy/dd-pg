use std::{rc::Rc, time::Duration};

use client_containers::{
    emoticons::{Emoticons, EmoticonsContainer},
    skins::{Skin, SkinContainer},
    weapons::{WeaponContainer, Weapons},
};
use client_render_base::render::{
    animation::AnimState,
    default_anim::{base_anim, idle_anim},
    tee::{offset_to_mid, RenderTee, TeeRenderHands, TeeRenderInfo, TeeRenderSkinColor},
    toolkit::ToolkitRender,
};
use egui::Rect;
use game_interface::types::{
    character_info::NetworkSkinInfo, emoticons::EmoticonType, render::character::TeeEye,
    resource_key::ResourceKey, weapons::WeaponType,
};
use graphics::{
    handles::{
        canvas::canvas::GraphicsCanvasHandle,
        stream::stream::{GraphicsStreamHandle, QuadStreamHandle},
        stream_types::StreamedQuad,
        texture::texture::TextureContainer,
    },
    streaming::quad_scope_begin,
};
use graphics_types::rendering::State;
use hiarc::hi_closure;
use math::math::vector::{dvec2, ubvec4, vec2};
use ui_base::{custom_callback::CustomCallbackTrait, types::UiState};

/// TODO: this function exists in the editor already. graphics also have a similar one.
pub fn rotate(center: &vec2, rotation: f32, points: &mut [vec2]) {
    let c = rotation.cos();
    let s = rotation.sin();

    for point in points.iter_mut() {
        let x = point.x - center.x;
        let y = point.y - center.y;
        *point = vec2 {
            x: x * c - y * s + center.x,
            y: x * s + y * c + center.y,
        };
    }
}

pub fn render_tee_for_ui(
    canvas_handle: &GraphicsCanvasHandle,
    skin_container: &mut SkinContainer,
    render_tee: &RenderTee,
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    render_rect: Rect,
    clip_rect: Option<Rect>,
    skin: &ResourceKey,
    skin_info: Option<&NetworkSkinInfo>,
    pos: vec2,
    size: f32,
    eyes: TeeEye,
) {
    #[derive(Debug)]
    struct RenderTeeCB {
        render_rect: Rect,
        clip_rect: Option<Rect>,
        skin: Rc<Skin>,
        skin_info: Option<NetworkSkinInfo>,
        pos: vec2,
        size: f32,
        canvas_handle: GraphicsCanvasHandle,
        render_tee: RenderTee,
        eyes: TeeEye,
        opacity: f32,
    }
    impl CustomCallbackTrait for RenderTeeCB {
        fn render(&self) {
            let mut anim_state = AnimState::default();
            anim_state.set(&base_anim(), &Duration::from_millis(0));
            anim_state.add(&idle_anim(), &Duration::from_millis(0), 1.0);

            let (color_body, color_feet) = if let Some(NetworkSkinInfo::Custom {
                body_color,
                feet_color,
            }) = self.skin_info
            {
                (body_color.into(), feet_color.into())
            } else {
                (TeeRenderSkinColor::Original, TeeRenderSkinColor::Original)
            };

            let tee_render_info = TeeRenderInfo {
                color_body,
                color_feet,
                got_air_jump: false,
                feet_flipped: false,
                size: self.size,
                eye_left: self.eyes,
                eye_right: self.eyes,
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
                &self.skin,
                &tee_render_info,
                &TeeRenderHands {
                    left: None,
                    right: None,
                },
                &dir,
                &(self.pos + offset_to_mid(&self.skin.metrics, &anim_state, &tee_render_info)),
                self.opacity,
                &state,
            );
        }
    }

    let skin = skin_container.get_or_default(skin);
    let cb = RenderTeeCB {
        render_rect,
        clip_rect,
        skin: skin.clone(),
        skin_info: skin_info.copied(),
        pos,
        size,
        canvas_handle: canvas_handle.clone(),
        render_tee: render_tee.clone(),
        eyes,
        opacity: ui.opacity(),
    };

    ui_state.add_custom_paint(ui, render_rect, Rc::new(cb));
}

pub fn render_weapon_for_ui(
    canvas_handle: &GraphicsCanvasHandle,
    weapon_container: &mut WeaponContainer,
    render_toolkit: &ToolkitRender,
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    render_rect: Rect,
    clip_rect: Option<Rect>,
    weapon: &ResourceKey,
    weapon_ty: WeaponType,
    pos: vec2,
    size: f32,
) {
    #[derive(Debug)]
    struct RenderWeaponCB {
        render_rect: Rect,
        clip_rect: Option<Rect>,
        weapons: Weapons,
        weapon_ty: WeaponType,
        pos: vec2,
        size: f32,
        canvas_handle: GraphicsCanvasHandle,
        render_toolkit: ToolkitRender,
        opacity: f32,
    }
    impl CustomCallbackTrait for RenderWeaponCB {
        fn render(&self) {
            let mut state = quad_scope_begin();
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

            state.set_colors_from_single(1.0, 1.0, 1.0, self.opacity);

            self.render_toolkit.render_weapon(
                &self.weapons,
                &self.weapon_ty,
                &self.pos,
                self.size,
                &dvec2::new(1.0, 0.0),
                state,
            );
        }
    }

    let weapon = weapon_container.get_or_default(weapon);
    let cb = RenderWeaponCB {
        render_rect,
        clip_rect,
        weapons: weapon.clone(),
        weapon_ty,
        pos,
        size,
        canvas_handle: canvas_handle.clone(),
        render_toolkit: render_toolkit.clone(),
        opacity: ui.opacity(),
    };

    ui_state.add_custom_paint(ui, render_rect, Rc::new(cb));
}

pub fn render_emoticon_for_ui(
    stream_handle: &GraphicsStreamHandle,
    canvas_handle: &GraphicsCanvasHandle,
    emoticon_container: &mut EmoticonsContainer,
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    render_rect: Rect,
    clip_rect: Option<Rect>,
    emoticon: &ResourceKey,
    pos: vec2,
    size: f32,
    ty: EmoticonType,
) {
    #[derive(Debug)]
    struct RenderEmoticonCB {
        render_rect: Rect,
        clip_rect: Option<Rect>,
        emoticons: Emoticons,
        pos: vec2,
        size: f32,
        canvas_handle: GraphicsCanvasHandle,
        stream_handle: GraphicsStreamHandle,
        ty: EmoticonType,
    }
    impl CustomCallbackTrait for RenderEmoticonCB {
        fn render(&self) {
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

            pub fn render_rect(
                stream_handle: &GraphicsStreamHandle,
                center: &vec2,
                size: &vec2,
                state: State,
                texture: &TextureContainer,
            ) {
                stream_handle.render_quads(
                    hi_closure!([
                        center: &vec2,
                        size: &vec2,
                        texture: &TextureContainer
                    ], |mut stream_handle: QuadStreamHandle<'_>| -> () {
                        stream_handle.set_texture(texture);
                        stream_handle
                            .add_vertices(
                                StreamedQuad::new()
                                .from_pos_and_size(
                                    vec2::new(
                                        center.x - size.x / 2.0,
                                        center.y - size.y / 2.0
                                    ),
                                    *size
                                )
                                .color(ubvec4::new(255, 255, 255, 255))
                                .tex_free_form(
                                    vec2::new(0.0, 0.0),
                                    vec2::new(1.0, 0.0),
                                    vec2::new(1.0, 1.0),
                                    vec2::new(0.0, 1.0),
                                )
                                .into()
                            );
                    }),
                    state,
                );
            }

            render_rect(
                &self.stream_handle,
                &self.pos,
                &vec2::new(self.size, self.size),
                state,
                &self.emoticons.emoticons[self.ty as usize],
            );
        }
    }

    let emoticons = emoticon_container.get_or_default(emoticon);
    let cb = RenderEmoticonCB {
        render_rect,
        clip_rect,
        emoticons: emoticons.clone(),
        pos,
        size,
        canvas_handle: canvas_handle.clone(),
        stream_handle: stream_handle.clone(),
        ty,
    };

    ui_state.add_custom_paint(ui, render_rect, Rc::new(cb));
}
