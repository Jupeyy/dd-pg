use std::time::Duration;

use client_containers::{container::ContainerKey, hud::HudContainer, weapons::WeaponContainer};
use client_render::hud::page::{HudRender, HudRenderPipe};
use game_interface::types::{
    game::{GameTickType, NonZeroGameTickType},
    render::{character::LocalCharacterRenderInfo, game::GameRenderInfo},
    weapons::{WeaponType, NUM_WEAPONS},
};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        canvas::canvas::GraphicsCanvasHandle,
        quad_container::quad_container::{QuadContainer, QuadContainerRenderCount},
    },
    quad_container::Quad,
    streaming::quad_scope_begin,
};
use graphics_types::rendering::State;
use math::math::{vector::vec2, Rng, RngSlice};
use ui_base::ui::UiCreator;

pub struct RenderHudPipe<'a> {
    pub hud_container: &'a mut HudContainer,
    pub weapon_container: &'a mut WeaponContainer,
    pub local_player_render_info: &'a LocalCharacterRenderInfo,
    pub cur_weapon: WeaponType,
    pub race_timer_counter: &'a GameTickType,
    pub ticks_per_second: &'a NonZeroGameTickType,
    pub cur_time: &'a Duration,
    pub game: Option<&'a GameRenderInfo>,
}

pub struct RenderHud {
    quad_container: QuadContainer,

    ui: HudRender,

    heart_offset: usize,
    shield_offset: usize,
    weapon_ammo_offsets: [usize; NUM_WEAPONS],

    canvas_handle: GraphicsCanvasHandle,

    rng: Rng,
}

impl RenderHud {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let mut quads: Vec<Quad> = Default::default();

        let x = 5.0;
        let y = 5.0;

        // ammo of the different weapons
        let weapon_ammo_offsets = (0..NUM_WEAPONS)
            .map(|_| {
                let offset = quads.len();
                quads.extend((0..10).map(|index| {
                    Quad::new()
                        .from_rect(x + index as f32 * 12.0 + 1.0, y + 24.0, 10.0, 10.0)
                        .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0))
                }));
                offset
            })
            .collect::<Vec<_>>();

        // hearts
        let heart_offset = quads.len();
        quads.extend((0..10).map(|index| {
            Quad::new()
                .from_rect(x + index as f32 * 12.0, y, 12.0, 12.0)
                .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0))
        }));

        // shields
        let shield_offset = quads.len();
        quads.extend((0..10).map(|index| {
            Quad::new()
                .from_rect(x + index as f32 * 12.0, y + 12.0, 12.0, 12.0)
                .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0))
        }));

        let heart_shield_quad_container =
            graphics.quad_container_handle.create_quad_container(quads);

        Self {
            quad_container: heart_shield_quad_container,
            heart_offset,
            shield_offset,

            weapon_ammo_offsets: weapon_ammo_offsets.try_into().unwrap(),

            canvas_handle: graphics.canvas_handle.clone(),

            rng: Rng::new(0),

            ui: HudRender::new(graphics, creator),
        }
    }

    pub fn render(&mut self, pipe: &mut RenderHudPipe) {
        self.ui.render(&mut HudRenderPipe {
            cur_time: pipe.cur_time,
            race_timer_counter: pipe.race_timer_counter,
            ticks_per_second: pipe.ticks_per_second,
            game: pipe.game,
        });

        let hud = pipe
            .hud_container
            .get_or_default::<ContainerKey>(&"TODO".try_into().unwrap());
        let weapon = pipe
            .weapon_container
            .get_or_default::<ContainerKey>(&"TODO".try_into().unwrap());
        let mut state = State::default();
        state.map_canvas(0.0, 0.0, 300.0 * self.canvas_handle.canvas_aspect(), 300.0);

        let mut draw_scope = quad_scope_begin();
        draw_scope.set_state(&state);
        draw_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);

        // render heart
        let cur_health = pipe.local_player_render_info.health.min(10) as usize;
        let texture = &hud.heart;
        self.quad_container.render_quad_container(
            self.heart_offset,
            &QuadContainerRenderCount::Count(cur_health),
            0.0,
            0.0,
            1.0,
            1.0,
            draw_scope,
            texture.into(),
        );
        let texture = &hud.heart_empty;
        self.quad_container.render_quad_container(
            self.heart_offset + cur_health,
            &QuadContainerRenderCount::Count(10 - cur_health),
            0.0,
            0.0,
            1.0,
            1.0,
            draw_scope,
            texture.into(),
        );

        // render shields
        let cur_armor = pipe.local_player_render_info.armor.min(10) as usize;
        let texture = &hud.shield;
        self.quad_container.render_quad_container(
            self.shield_offset,
            &QuadContainerRenderCount::Count(cur_armor),
            0.0,
            0.0,
            1.0,
            1.0,
            draw_scope,
            texture.into(),
        );

        let texture = &hud.shield_empty;
        self.quad_container.render_quad_container(
            self.shield_offset + cur_armor,
            &QuadContainerRenderCount::Count(10 - cur_armor),
            0.0,
            0.0,
            1.0,
            1.0,
            draw_scope,
            texture.into(),
        );

        // render ammo
        let cur_weapon = weapon.by_type(pipe.cur_weapon);
        if !cur_weapon.projectiles.is_empty()
            && pipe.local_player_render_info.ammo_of_weapon.is_some()
        {
            let cur_ammo_of_weapon = pipe
                .local_player_render_info
                .ammo_of_weapon
                .unwrap()
                .min(10) as usize;
            let texture = cur_weapon.projectiles.random_entry(&mut self.rng);
            self.quad_container.render_quad_container(
                self.weapon_ammo_offsets[pipe.cur_weapon as usize],
                &QuadContainerRenderCount::Count(cur_ammo_of_weapon),
                0.0,
                0.0,
                1.0,
                1.0,
                draw_scope,
                texture.into(),
            );
        }
    }
}
