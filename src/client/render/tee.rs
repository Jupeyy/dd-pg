use graphics_base::{
    quad_container::{QuadContainerIndex, SQuad},
    streaming::quad_scope_begin,
};

use graphics_render_traits::GraphicsHandles;
use graphics_traits::GraphicsInterface;
use graphics_types::rendering::{ColorRGBA, State};
use math::math::{vector::vec2, PI};

use crate::containers::skins::{Skin, TeeSkinEye};

use super::animation::AnimState;

pub enum TeeRenderSkinTextures<'a> {
    Original(&'a Skin),
    Colorable(&'a Skin),
}

impl<'a> TeeRenderSkinTextures<'a> {
    pub fn unwrap(&self) -> &Skin {
        match self {
            TeeRenderSkinTextures::Original(textures) => textures,
            TeeRenderSkinTextures::Colorable(textures) => textures,
        }
    }
}

pub struct TeeRenderInfo<'a> {
    /*CTeeRenderInfo()
    {
        m_ColorBody = ColorRGBA(1, 1, 1);
        m_ColorFeet = ColorRGBA(1, 1, 1);
        m_Size = 1.0f;
        m_GotAirJump = 1;
        m_TeeRenderFlags = 0;
    };*/
    pub render_skin: TeeRenderSkinTextures<'a>,

    pub color_body: ColorRGBA,
    pub color_feet: ColorRGBA,
    /*CSkin::SSkinMetrics m_SkinMetrics;

    bool m_CustomColoredSkin;
    ColorRGBA m_BloodColor;

    float m_Size;*/
    pub got_air_jump: bool,
    /*
    int m_TeeRenderFlags;*/
    pub feet_flipped: bool,
    pub size: f32,
}

pub struct RenderTee {
    tee_quad_container: QuadContainerIndex,

    body_offset: usize,
    body_outline_offset: usize,
    eye_offset: usize,
    foot_offset: usize,
    foot_outline_offset: usize,
    mirrored_foot_offset: usize,
    mirrored_outline_foot_offset: usize,
}

pub const RENDER_TEE_BODY_SIZE_BASE: f32 = 64.0;

pub struct RenderTeeMath {
    pub body_pos: vec2,
    pub body_scale: vec2,
    pub body_rotation: f32,
    pub eye_left_pos: vec2,
    pub eye_left_scale: vec2,
    pub eye_left_rotation: f32,
    pub eye_right_pos: vec2,
    pub eye_right_scale: vec2,
    pub eye_right_rotation: f32,
    pub foot_left_pos: vec2,
    pub foot_left_scale: vec2,
    pub foot_left_rotation: f32,
    pub foot_right_pos: vec2,
    pub foot_right_scale: vec2,
    pub foot_right_rotation: f32,
}

impl RenderTee {
    pub fn new(graphics: &mut dyn GraphicsInterface) -> Self {
        let GraphicsHandles {
            quad_container_handle,
            ..
        } = graphics.get_handles();
        let tee_quad_container = quad_container_handle.create_quad_container(false);

        let body_offset = quad_container_handle.quad_container_add_quads(
            &tee_quad_container,
            &[SQuad::new().from_size_centered(RENDER_TEE_BODY_SIZE_BASE)],
        );
        let body_outline_offset = body_offset;

        let eye_offset = quad_container_handle.quad_container_add_quads(
            &tee_quad_container,
            &[SQuad::new().from_size_centered(64.0 * 0.4)],
        );

        let foot_offset = quad_container_handle.quad_container_add_quads(
            &tee_quad_container,
            &[SQuad::new().from_rect(-32.0, -16.0, 64.0, 32.0)],
        );
        let foot_outline_offset = foot_offset;

        let mirrored_foot_offset = quad_container_handle.quad_container_add_quads(
            &tee_quad_container,
            &[SQuad::new().from_rect(-32.0, -16.0, 64.0, 32.0).with_tex(&[
                vec2 { x: 1.0, y: 0.0 },
                vec2 { x: 0.0, y: 0.0 },
                vec2 { x: 0.0, y: 1.0 },
                vec2 { x: 1.0, y: 1.0 },
            ])],
        );
        let mirrored_outline_foot_offset = mirrored_foot_offset;

        quad_container_handle.quad_container_upload(&tee_quad_container);

        Self {
            tee_quad_container,
            body_offset,
            body_outline_offset,
            eye_offset,
            foot_offset,
            foot_outline_offset,
            mirrored_foot_offset,
            mirrored_outline_foot_offset,
        }
    }

    fn get_render_tee_anim_scale_and_base_size(
        _anim: &AnimState,
        info: &TeeRenderInfo,
        anim_scale: &mut f32,
        base_size: &mut f32,
    ) {
        *anim_scale = info.size * 1.0 / 64.0;
        *base_size = info.size;
    }

    fn get_render_tee_body_scale(base_size: f32, body_scale_x: &mut f32, body_scale_y: &mut f32) {
        let body_scale = base_size; // TODO: g_Config.m_ClFatSkins ? BaseSize * 1.3f : BaseSize;
        *body_scale_x = body_scale;
        *body_scale_y = body_scale;
        *body_scale_x /= 64.0;
        *body_scale_y /= 64.0;
    }

    pub fn render_tee_math(
        anim: &AnimState,
        info: &TeeRenderInfo,
        emote: TeeSkinEye,
        dir: &vec2,
        pos: &vec2,
    ) -> RenderTeeMath {
        let direction = *dir;
        let position = *pos;

        // general
        let mut anim_scale = 1.0;
        let mut base_size = 1.0;
        Self::get_render_tee_anim_scale_and_base_size(anim, info, &mut anim_scale, &mut base_size);

        // body
        let body_pos = position
            + vec2 {
                x: anim.body.pos.x,
                y: anim.body.pos.y,
            } * anim_scale;

        let mut body_scale_x = anim.body.scale.x;
        let mut body_scale_y = anim.body.scale.y;
        Self::get_render_tee_body_scale(base_size, &mut body_scale_x, &mut body_scale_y);
        let body_rotation = anim.body.rotation;

        // eye
        let eye_scale = base_size * 0.40;
        let eye_height = if emote == TeeSkinEye::Blink {
            base_size * 0.15
        } else {
            eye_scale
        };
        let eye_separation = (0.075 - 0.010 * direction.x.abs()) * base_size;
        let offset = vec2 {
            x: direction.x * 0.125,
            y: -0.05 + direction.y * 0.10,
        } * base_size;

        let eye_pos_left = vec2 {
            x: body_pos.x - eye_separation + offset.x,
            y: body_pos.y + offset.y,
        };

        let eye_pos_right = vec2 {
            x: body_pos.x + eye_separation + offset.x,
            y: body_pos.y + offset.y,
        };

        let eye_scale_left = vec2 {
            x: eye_scale / (64.0 * 0.4) * anim.left_eye.scale.x,
            y: eye_height / (64.0 * 0.4) * anim.left_eye.scale.y,
        };

        let eye_size_right = vec2 {
            x: -eye_scale / (64.0 * 0.4) * anim.right_eye.scale.x,
            y: eye_height / (64.0 * 0.4) * anim.right_eye.scale.y,
        };

        let eye_rotation_left = anim.left_eye.rotation;
        let eye_rotation_right = anim.right_eye.rotation;

        // feet
        let feet_width = base_size;
        let feet_height = base_size / 2.0;

        let foot = &anim.left_foot;
        let feet_pos_left = vec2 {
            x: position.x + foot.pos.x * anim_scale,
            y: position.y + foot.pos.y * anim_scale,
        };
        let foot_left_rotation = foot.rotation;

        let foot = &anim.right_foot;
        let feet_pos_right = vec2 {
            x: position.x + foot.pos.x * anim_scale,
            y: position.y + foot.pos.y * anim_scale,
        };
        let foot_right_rotation = foot.rotation;

        RenderTeeMath {
            body_pos: body_pos,
            body_scale: vec2 {
                x: body_scale_x,
                y: body_scale_y,
            },
            body_rotation: body_rotation,
            eye_left_pos: eye_pos_left,
            eye_left_scale: eye_scale_left,
            eye_left_rotation: eye_rotation_left,
            eye_right_pos: eye_pos_right,
            eye_right_scale: eye_size_right,
            eye_right_rotation: eye_rotation_right,
            foot_left_pos: feet_pos_left,
            foot_left_scale: vec2 {
                x: feet_width / 64.0,
                y: feet_height / 32.0,
            },
            foot_left_rotation: foot_left_rotation,
            foot_right_pos: feet_pos_right,
            foot_right_scale: vec2 {
                x: feet_width / 64.0,
                y: feet_height / 32.0,
            },
            foot_right_rotation: foot_right_rotation,
        }
    }

    pub fn render_tee_body(
        &self,
        graphics: &mut dyn GraphicsInterface,
        state: &State,
        body_pos: &vec2,
        body_scale: &vec2,
        body_rotation: f32,
        body_color: &ColorRGBA,
        alpha: f32,
        skin_textures: &Skin,
        outline: bool,
    ) {
        let mut quad_scope = quad_scope_begin();
        let GraphicsHandles {
            quad_container_handle,
            ..
        } = graphics.get_handles();
        quad_scope.set_state(state);
        quad_scope.set_rotation(body_rotation * PI * 2.0);

        // draw body
        quad_scope.set_colors_from_single(body_color.r, body_color.g, body_color.b, alpha);
        quad_scope.set_texture(if outline {
            &skin_textures.body_outline
        } else {
            &skin_textures.body
        });
        quad_container_handle.render_quad_container_as_sprite(
            &self.tee_quad_container,
            if !outline {
                self.body_offset
            } else {
                self.body_outline_offset
            },
            body_pos.x,
            body_pos.y,
            body_scale.x,
            body_scale.y,
            quad_scope,
        );
    }

    pub fn render_tee_eyes(
        &self,
        graphics: &mut dyn GraphicsInterface,
        state: &State,
        emote: TeeSkinEye,
        eye_left_pos: &vec2,
        eye_left_scale: &vec2,
        eye_left_rotation: f32,
        eye_right_pos: &vec2,
        eye_right_scale: &vec2,
        eye_right_rotation: f32,
        eye_left_color: &ColorRGBA,
        eye_right_color: &ColorRGBA,
        alpha: f32,
        skin_textures: &Skin,
    ) {
        let quad_offset = self.eye_offset;
        let tee_eye_index = emote as usize - TeeSkinEye::Normal as usize;

        let mut quad_scope = quad_scope_begin();
        let GraphicsHandles {
            quad_container_handle,
            ..
        } = graphics.get_handles();
        quad_scope.set_state(state);
        quad_scope.set_rotation(eye_left_rotation * PI * 2.0);
        quad_scope.set_texture(&skin_textures.left_eyes[tee_eye_index]);
        quad_scope.set_colors_from_single(
            eye_left_color.r,
            eye_left_color.g,
            eye_left_color.b,
            alpha,
        );
        quad_container_handle.render_quad_container_as_sprite(
            &self.tee_quad_container,
            quad_offset,
            eye_left_pos.x,
            eye_left_pos.y,
            eye_left_scale.x,
            eye_left_scale.y,
            quad_scope,
        );
        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(state);
        quad_scope.set_rotation(eye_right_rotation * PI * 2.0);
        quad_scope.set_texture(&skin_textures.right_eyes[tee_eye_index]);
        quad_scope.set_colors_from_single(
            eye_right_color.r,
            eye_right_color.g,
            eye_right_color.b,
            alpha,
        );
        quad_container_handle.render_quad_container_as_sprite(
            &self.tee_quad_container,
            quad_offset,
            eye_right_pos.x,
            eye_right_pos.y,
            eye_right_scale.x,
            eye_right_scale.y,
            quad_scope,
        );
    }

    pub fn render_tee_feet(
        &self,
        graphics: &mut dyn GraphicsInterface,
        state: &State,
        _emote: TeeSkinEye,
        dir: &vec2,
        foot_left_pos: &vec2,
        foot_left_scale: &vec2,
        foot_left_rotation: f32,
        foot_right_pos: &vec2,
        foot_right_scale: &vec2,
        foot_right_rotation: f32,
        foot_left_color: &ColorRGBA,
        foot_right_color: &ColorRGBA,
        alpha: f32,
        skin_textures: &Skin,
        outline: bool,
        flipped: bool,
        got_air_jump: bool,
        is_left_foot: bool,
    ) {
        let mut quad_offset = if !outline {
            self.foot_offset
        } else {
            self.foot_outline_offset
        };
        if dir.x < 0.0 && flipped {
            quad_offset = if !outline {
                self.mirrored_foot_offset
            } else {
                self.mirrored_outline_foot_offset
            };
        }

        let foot_rotation = if is_left_foot {
            foot_left_rotation
        } else {
            foot_right_rotation
        };

        let mut quad_scope = quad_scope_begin();
        let GraphicsHandles {
            quad_container_handle,
            ..
        } = graphics.get_handles();
        quad_scope.set_state(state);
        quad_scope.set_rotation(foot_rotation * PI * 2.0);

        let indicate = !got_air_jump; // TODO: && g_Config.m_ClAirjumpindicator;
        let mut color_scale = 1.0;

        if !outline {
            if indicate {
                color_scale = 0.5;
            }
        }

        let foot_color = if is_left_foot {
            foot_left_color
        } else {
            foot_right_color
        };

        quad_scope.set_colors_from_single(
            foot_color.r * color_scale,
            foot_color.g * color_scale,
            foot_color.b * color_scale,
            alpha,
        );

        quad_scope.set_texture(if outline {
            if !is_left_foot {
                &skin_textures.left_foot_outline
            } else {
                &skin_textures.right_foot_outline
            }
        } else {
            if !is_left_foot {
                &skin_textures.left_foot
            } else {
                &skin_textures.right_foot
            }
        });
        let foot_pos = if is_left_foot {
            foot_left_pos
        } else {
            foot_right_pos
        };
        let foot_scale = if is_left_foot {
            foot_left_scale
        } else {
            foot_right_scale
        };
        quad_container_handle.render_quad_container_as_sprite(
            &self.tee_quad_container,
            quad_offset,
            foot_pos.x,
            foot_pos.y,
            foot_scale.x,
            foot_scale.y,
            quad_scope,
        );
    }

    pub fn render_tee_from_math(
        &self,
        tee_math: &RenderTeeMath,
        graphics: &mut dyn GraphicsInterface,
        info: &TeeRenderInfo,
        emote: TeeSkinEye,
        dir: &vec2,
        alpha: f32,
        state: &State,
    ) {
        let RenderTeeMath {
            body_pos,
            body_scale,
            body_rotation,
            eye_left_pos,
            eye_left_scale,
            eye_left_rotation,
            eye_right_pos,
            eye_right_scale,
            eye_right_rotation,
            foot_left_pos,
            foot_left_scale,
            foot_left_rotation,
            foot_right_pos,
            foot_right_scale,
            foot_right_rotation,
        } = tee_math;

        let skin_textures = info.render_skin.unwrap();

        // TODO: render tee hands
        /*
        fn RenderHand(CTeeRenderInfo *pInfo, vec2 CenterPos, vec2 dir, float AngleOffset, vec2 PostRotOffset, float Alpha)
        {
            vec2 HandPos = CenterPos + dir;
            float Angle = angle(dir);
            if(dir.x < 0)
                Angle -= AngleOffset;
            else
                Angle += AngleOffset;

            vec2 DirX = dir;
            vec2 DirY(-dir.y, dir.x);

            if(dir.x < 0)
                DirY = -DirY;

            HandPos += DirX * PostRotOffset.x;
            HandPos += DirY * PostRotOffset.y;

            const CSkin::SSkinTextures *pSkinTextures = pInfo->m_CustomColoredSkin ? &pInfo->m_ColorableRenderSkin : &pInfo->m_OriginalRenderSkin;

            Graphics()->SetColor(pInfo->m_ColorBody.r, pInfo->m_ColorBody.g, pInfo->m_ColorBody.b, Alpha);

            // two passes
            for(int i = 0; i < 2; i++)
            {
                int QuadOffset = NUM_WEAPONS * 2 + i;
                Graphics()->QuadsSetRotation(Angle);
                Graphics()->TextureSet(i == 0 ? pSkinTextures->m_HandsOutline : pSkinTextures->m_Hands);
                Graphics()->RenderQuadContainerAsSprite(m_WeaponEmoteQuadContainerIndex, QuadOffset, HandPos.x, HandPos.y);
            }
        } */

        // first pass we draw the outline
        // second pass we draw the filling
        for p in 0..2 {
            let outline = if p == 0 { 1 } else { 0 };

            for f in 0..2 {
                if f == 1 {
                    // draw body
                    self.render_tee_body(
                        graphics,
                        state,
                        body_pos,
                        body_scale,
                        *body_rotation,
                        &info.color_body,
                        alpha,
                        skin_textures,
                        outline == 1,
                    );

                    // draw eyes
                    if p == 1 {
                        self.render_tee_eyes(
                            graphics,
                            state,
                            emote,
                            eye_left_pos,
                            eye_left_scale,
                            *eye_left_rotation,
                            eye_right_pos,
                            eye_right_scale,
                            *eye_right_rotation,
                            &info.color_body,
                            &info.color_body,
                            alpha,
                            skin_textures,
                        );
                    }
                }

                // draw feet
                self.render_tee_feet(
                    graphics,
                    state,
                    emote,
                    dir,
                    foot_left_pos,
                    foot_left_scale,
                    *foot_left_rotation,
                    foot_right_pos,
                    foot_right_scale,
                    *foot_right_rotation,
                    &info.color_feet,
                    &info.color_feet,
                    alpha,
                    skin_textures,
                    outline == 1,
                    info.feet_flipped,
                    info.got_air_jump,
                    f == 0,
                )
            }
        }
    }

    pub fn render_tee(
        &self,
        graphics: &mut dyn GraphicsInterface,
        anim: &AnimState,
        info: &TeeRenderInfo,
        emote: TeeSkinEye,
        dir: &vec2,
        pos: &vec2,
        alpha: f32,
        state: &State,
    ) {
        self.render_tee_from_math(
            &Self::render_tee_math(anim, info, emote, dir, pos),
            graphics,
            info,
            emote,
            dir,
            alpha,
            state,
        )
    }
}
