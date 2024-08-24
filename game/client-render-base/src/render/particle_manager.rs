#![allow(unused)]

use std::{cell::Cell, collections::VecDeque, sync::Arc, time::Duration};

use crate::{map::render_pipe::Camera, render::canvas_mapping::CanvasMappingIngame};
use client_containers::{container::ContainerKey, particles::ParticlesContainer};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        quad_container::quad_container::{QuadContainer, QuadContainerRenderCount},
        stream::stream::{GraphicsStreamHandle, StreamedSprites, StreamedUniforms},
        texture::texture::TextureType,
    },
    quad_container::Quad,
    streaming::quad_scope_begin,
};
use graphics_types::{
    commands::{RenderSpriteInfo, GRAPHICS_MAX_UNIFORM_RENDER_COUNT},
    rendering::{ColorRGBA, State},
};
use hiarc::{hi_closure, Hiarc};
use math::math::{mix, vector::vec2, Rng};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use shared_game::collision::collision::Collision;

use super::particle::Particle;

const MAX_PARTICLES: usize = 1024 * 8;

#[derive(Copy, Hiarc, Clone, PartialEq, FromPrimitive)]
pub enum ParticleGroup {
    ProjectileTrail = 0,
    Explosions,
    Extra,
    General,

    // must stay last
    Count,
}

#[derive(Debug, Hiarc)]
pub struct ParticleManager {
    particle_quad_container: QuadContainer,
    canvas_mapping: CanvasMappingIngame,
    stream_handle: GraphicsStreamHandle,

    particle_groups: [VecDeque<Particle>; ParticleGroup::Count as usize],

    // TODO: wtf is this?
    friction_fraction: f32,

    last_time: Duration,
    /// 5 times per second
    pub last_5_time: Duration,
    /// 50 times per second
    pub last_50_time: Duration,
    /// 100 times per second
    pub last_100_time: Duration,

    pub rng: Rng,
}

impl ParticleManager {
    pub fn new(graphics: &Graphics, cur_time: &Duration) -> Self {
        let particle_quad_container = graphics
            .quad_container_handle
            .create_quad_container([Quad::new().from_size_centered(1.0)].into());

        Self {
            particle_quad_container,
            canvas_mapping: CanvasMappingIngame::new(graphics),
            stream_handle: graphics.stream_handle.clone(),

            particle_groups: Default::default(),
            friction_fraction: 0.0,

            last_time: *cur_time,
            last_5_time: Duration::from_nanos(
                ((cur_time.as_nanos() / Duration::from_millis(1000 / 5).as_nanos())
                    * Duration::from_millis(1000 / 5).as_nanos()) as u64,
            ),
            last_50_time: Duration::from_nanos(
                ((cur_time.as_nanos() / Duration::from_millis(1000 / 50).as_nanos())
                    * Duration::from_millis(1000 / 50).as_nanos()) as u64,
            ),
            last_100_time: Duration::from_nanos(
                ((cur_time.as_nanos() / Duration::from_millis(1000 / 100).as_nanos())
                    * Duration::from_millis(1000 / 100).as_nanos()) as u64,
            ),

            rng: Rng::new(0),
        }
    }

    fn reset(&mut self) {
        // reset particles
        self.particle_groups.iter_mut().for_each(|p| p.clear());
    }

    pub fn add(&mut self, group: ParticleGroup, mut part: Particle, time_passed: f32) {
        part.life = time_passed;
        self.particle_groups[group as usize].push_back(part);
    }

    pub fn update_rates(&mut self) {
        let next_5 = Duration::from_nanos(
            ((self.last_time.as_nanos() / Duration::from_millis(1000 / 5).as_nanos())
                * Duration::from_millis(1000 / 5).as_nanos()) as u64,
        );
        let offset_5 = Duration::from_millis(1000 / 5);
        if next_5 >= self.last_5_time {
            self.last_5_time = next_5 + offset_5;
        }

        let next_50 = Duration::from_nanos(
            ((self.last_time.as_nanos() / Duration::from_millis(1000 / 50).as_nanos())
                * Duration::from_millis(1000 / 50).as_nanos()) as u64,
        );
        let offset_50 = Duration::from_millis(1000 / 50);
        if next_50 >= self.last_50_time {
            self.last_50_time = next_50 + offset_50;
        }

        let next_100 = Duration::from_nanos(
            ((self.last_time.as_nanos() / Duration::from_millis(1000 / 100).as_nanos())
                * Duration::from_millis(1000 / 100).as_nanos()) as u64,
        );
        let offset_100 = Duration::from_millis(1000 / 100);
        if next_100 >= self.last_100_time {
            self.last_100_time = next_100 + offset_100;
        }
    }

    pub fn update(&mut self, cur_time: &Duration, collision: &Collision) {
        let time_passed_dur = *cur_time - self.last_time;
        self.last_time = *cur_time;
        if time_passed_dur.is_zero() {
            return;
        }

        let time_passed = time_passed_dur.as_secs_f32();

        self.friction_fraction += time_passed;

        if self.friction_fraction > 2.0 {
            // safety measure
            self.friction_fraction = 0.0;
        }

        let mut friction_count = 0;
        while self.friction_fraction > 0.05 {
            friction_count += 1;
            self.friction_fraction -= 0.05;
        }

        self.particle_groups.iter_mut().for_each(|particle_group| {
            particle_group.retain_mut(|particle| {
                particle.vel.y += particle.gravity * time_passed;

                for _ in 0..friction_count {
                    // apply friction
                    particle.vel *= particle.friction;
                }

                // move the point
                let old_life = particle.life;
                particle.life += time_passed;
                let life_diff_vel = particle.life.min(particle.max_lifetime_vel)
                    - old_life.min(particle.max_lifetime_vel);
                if old_life < particle.max_lifetime_vel {
                    let mut vel = particle.vel * life_diff_vel;
                    if particle.collides {
                        let mut bounces = 0;
                        let mut pos = particle.pos * 32.0;
                        let mut inout_vel = vel * 32.0;
                        collision.move_point(
                            &mut pos,
                            &mut inout_vel,
                            0.1 + 0.9 * self.rng.random_float(),
                            &mut bounces,
                        );
                        particle.pos = pos / 32.0;
                        vel = inout_vel / 32.0;
                    } else {
                        particle.pos += vel;
                    }
                    particle.vel = vel * (1.0 / life_diff_vel);
                }

                particle.rot += time_passed * particle.rot_speed;

                // check particle death
                particle.life <= particle.life_span
            })
        });
    }

    fn particle_is_visible_on_screen(state: &State, cur_pos: &vec2, mut cur_size: f32) -> bool {
        let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();

        // for simplicity assume the worst case rotation, that increases the bounding box around the particle by its diagonal
        let sqrt_of_2 = 2.0_f32.sqrt();
        cur_size *= sqrt_of_2;

        // always uses the mid of the particle
        let size_half = cur_size / 2.0;

        cur_pos.x + size_half >= canvas_x0
            && cur_pos.x - size_half <= canvas_x1
            && cur_pos.y + size_half >= canvas_y0
            && cur_pos.y - size_half <= canvas_y1
    }

    pub fn render_group(
        &self,
        group: ParticleGroup,
        particle_container: &mut ParticlesContainer,
        camera: &Camera,
    ) {
        if !self.particle_groups[group as usize].is_empty() {
            let mut state = State::new();
            let center = camera.pos;
            self.canvas_mapping.map_canvas_for_ingame_items(
                &mut state,
                center.x,
                center.y,
                camera.zoom,
            );

            let p = &self.particle_groups[group as usize][0];
            let mut alpha = p.color.a;
            if p.use_alpha_fading {
                let a = p.life / p.life_span;
                alpha = mix(&p.start_alpha, &p.end_alpha, a);
            }
            // batching makes sense for stuff like ninja particles
            let last_color = Cell::new(ColorRGBA {
                r: p.color.r,
                g: p.color.g,
                b: p.color.b,
                a: alpha,
            });

            let last_texture = Cell::new(p.texture);

            let particle_quad_container = &self.particle_quad_container;
            let state = &state;
            let particle_groups = &self.particle_groups;
            let last_color = &last_color;
            let last_texture = &last_texture;
            self.stream_handle.fill_sprites_uniform_instance(
                hi_closure!([
                    particle_groups: &[VecDeque<Particle>; ParticleGroup::Count as usize],
                    group: ParticleGroup,
                    state: &State,
                    last_color: &Cell<ColorRGBA>,
                    alpha: f32,
                    last_texture: &Cell<&'static str>,
                ], |mut stream_handle: StreamedSprites<'_>| -> () {
                    for p in particle_groups[group as usize].iter() {
                        let a = p.life / p.life_span;
                        let ppos = p.pos;
                        let size = mix(&p.start_size, &p.end_size, a);
                        let mut alpha = p.color.a;
                        if p.use_alpha_fading {
                            alpha = mix(&p.start_alpha, &p.end_alpha, a);
                        }

                        let texture = p.texture;

                        // the current position, respecting the size, is inside the viewport, render it, else ignore
                        if ParticleManager::particle_is_visible_on_screen(state, &ppos, size) {
                            let last_color_cmp = last_color.get();
                            if last_color_cmp.r != p.color.r
                                || last_color_cmp.g != p.color.g
                                || last_color_cmp.b != p.color.b
                                || last_color_cmp.a != alpha
                                || !last_texture.get().eq(texture)
                            {
                                stream_handle.flush();

                                last_texture.set(texture);

                                last_color.set(ColorRGBA {
                                    r: p.color.r,
                                    g: p.color.g,
                                    b: p.color.b,
                                    a: alpha,
                                })
                            }

                            stream_handle.add(RenderSpriteInfo {
                                pos: ppos,
                                scale: size,
                                rotation: p.rot,
                            });
                        }
                    }
                }),
                hi_closure!([last_texture: &Cell<&'static str>, last_color: &Cell<ColorRGBA>, particle_quad_container: &QuadContainer, state: &State, particle_container: &mut ParticlesContainer], |instance: usize, particle_count: usize| -> () {
                    let part_texture = particle_container
                        .get_or_default::<ContainerKey>(&"TODO".try_into().unwrap())
                        .get_by_name(last_texture.get())
                        .clone();

                    let mut quad_scope = quad_scope_begin();
                    quad_scope.set_state(state);
                    let last_color = last_color.get();
                    quad_scope.set_colors_from_single(
                        last_color.r,
                        last_color.g,
                        last_color.b,
                        last_color.a,
                    );
                    particle_quad_container
                        .render_quad_container_as_sprite_multiple(
                            0,
                            instance,
                            particle_count,
                            quad_scope,
                            part_texture.into(),
                        );
                })
            );
        }
    }

    pub fn render_groups(
        &self,
        start_group: ParticleGroup,
        particle_container: &mut ParticlesContainer,
        camera: &Camera,
    ) {
        for i in start_group as usize..ParticleGroup::Count as usize {
            self.render_group(
                ParticleGroup::from_usize(i).unwrap(),
                particle_container,
                camera,
            );
        }
    }
}
