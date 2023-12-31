#![allow(unused)]

use std::{collections::VecDeque, sync::Arc, time::Duration};

use base::system::SystemTimeInterface;
use base_fs::filesys::FileSystem;
use base_io::{io::IO, io_batcher::IOBatcher};
use client_containers::particles::ParticlesContainer;
use client_render_base::{
    map::render_pipe::Camera, render::canvas_mapping::map_canvas_for_ingame_items,
};
use graphics::{
    graphics::Graphics,
    handles::quad_container::{QuadContainer, QuadContainerRenderCount},
    quad_container::Quad,
    streaming::quad_scope_begin,
};
use graphics_types::{
    commands::{RenderSpriteInfo, GRAPHICS_MAX_UNIFORM_RENDER_COUNT},
    rendering::{ColorRGBA, State},
};
use math::math::{mix, random_float, vector::vec2};
use shared_game::collision::collision::Collision;

use super::particle::Particle;

const MAX_PARTICLES: usize = 1024 * 8;

#[derive(Copy, Clone, PartialEq)]
pub enum ParticleGroup {
    ProjectileTrail = 0,
    Explosions,
    Extra,
    General,

    // must stay last
    Count,
}

pub struct ParticleManager {
    particle_quad_container: QuadContainer,

    particle_groups: [VecDeque<Particle>; ParticleGroup::Count as usize],

    // TODO: wtf is this?
    friction_fraction: f32,

    last_time: Duration,
}

impl ParticleManager {
    pub fn new(graphics: &mut Graphics, sys: &dyn SystemTimeInterface) -> Self {
        let particle_quad_container = graphics
            .quad_container_handle
            .create_quad_container([Quad::new().from_size_centered(1.0)].into());

        Self {
            particle_quad_container,
            particle_groups: Default::default(),
            friction_fraction: 0.0,

            last_time: sys.time_get_nanoseconds(),
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
                let mut vel = particle.vel * time_passed;
                if particle.collides {
                    let mut bounces = 0;
                    collision.move_point(
                        &mut particle.pos,
                        &mut vel,
                        0.1 + 0.9 * random_float(),
                        &mut bounces,
                    );
                } else {
                    particle.pos += vel;
                }
                particle.vel = vel * (1.0 / time_passed);

                particle.life += time_passed;
                particle.rot += time_passed * particle.rot_speed;

                // check particle death
                if particle.life > particle.life_span {
                    false
                } else {
                    true
                }
            })
        });
    }

    fn particle_is_visible_on_screen(
        &self,
        state: &State,
        cur_pos: &vec2,
        mut cur_size: f32,
    ) -> bool {
        let (canvas_x0, canvas_y0, canvas_x1, canvas_y1) = state.get_canvas_mapping();

        // for simplicity assume the worst case rotation, that increases the bounding box around the particle by its diagonal
        let sqrt_of_2 = (2.0 as f32).sqrt();
        cur_size = sqrt_of_2 * cur_size;

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
        graphics: &mut Graphics,
        io: &IO,
        runtime_thread_pool: &Arc<rayon::ThreadPool>,
        camera: &Camera,
    ) {
        if !self.particle_groups[group as usize].is_empty() {
            let mut state = State::new();
            let center = camera.pos;
            map_canvas_for_ingame_items(graphics, &mut state, center.x, center.y, camera.zoom);
            let mut particle_render_info = graphics.stream_handle.get_sprites_uniform_instance();
            let (mut particles, mut used_count, mut instance) = particle_render_info.get();

            let p = &self.particle_groups[group as usize][0];
            let mut alpha = p.color.a;
            if p.use_alpha_fading {
                let a = p.life / p.life_span;
                alpha = mix(&p.start_alpha, &p.end_alpha, a);
            }
            // batching makes sense for stuff like ninja particles
            let mut last_color = ColorRGBA::default();
            last_color.r = p.color.r;
            last_color.g = p.color.g;
            last_color.b = p.color.b;
            last_color.a = alpha;

            let mut last_texture = p.texture;

            for p in self.particle_groups[group as usize].iter() {
                let a = p.life / p.life_span;
                let ppos = p.pos;
                let size = mix(&p.start_size, &p.end_size, a);
                let mut alpha = p.color.a;
                if p.use_alpha_fading {
                    alpha = mix(&p.start_alpha, &p.end_alpha, a);
                }

                let texture = p.texture;

                // the current position, respecting the size, is inside the viewport, render it, else ignore
                if self.particle_is_visible_on_screen(&state, &ppos, size) {
                    if *used_count == GRAPHICS_MAX_UNIFORM_RENDER_COUNT
                        || last_color.r != p.color.r
                        || last_color.g != p.color.g
                        || last_color.b != p.color.b
                        || last_color.a != alpha
                        || !last_texture.eq(texture)
                    {
                        let particle_count = *used_count;
                        drop(particle_render_info);

                        let part_texture = particle_container
                            .get_or_default(&p.texture)
                            .get_by_name(&p.texture)
                            .clone();

                        let mut quad_scope = quad_scope_begin();
                        quad_scope.set_state(&state.clone());
                        quad_scope.set_texture(&part_texture);
                        quad_scope.set_colors_from_single(
                            last_color.r,
                            last_color.g,
                            last_color.b,
                            last_color.a,
                        );
                        self.particle_quad_container
                            .render_quad_container_as_sprite_multiple(
                                0,
                                instance,
                                particle_count,
                                quad_scope,
                            );
                        particle_render_info =
                            graphics.stream_handle.get_sprites_uniform_instance();
                        (particles, used_count, instance) = particle_render_info.get();

                        last_texture = texture;

                        last_color.r = p.color.r;
                        last_color.g = p.color.g;
                        last_color.b = p.color.b;
                        last_color.a = alpha;
                    }

                    particles[*used_count] = RenderSpriteInfo {
                        pos: ppos,
                        scale: size,
                        rotation: p.rot,
                    };
                    *used_count += 1;
                }
            }

            let particle_count = *used_count;
            drop(particle_render_info);

            let part_texture = particle_container
                .get_or_default(&p.texture)
                .get_by_name(&p.texture)
                .clone();

            let mut quad_scope = quad_scope_begin();
            quad_scope.set_state(&state.clone());
            quad_scope.set_texture(&part_texture);
            quad_scope.set_colors_from_single(
                last_color.r,
                last_color.g,
                last_color.b,
                last_color.a,
            );
            self.particle_quad_container
                .render_quad_container_as_sprite_multiple(0, instance, particle_count, quad_scope);
        }
    }
}
