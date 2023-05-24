use std::time::Duration;

use bincode::Encode;
use egui::{
    epaint::{
        self,
        ahash::{HashMap, HashMapExt},
        Primitive,
    },
    Color32, ImageData, TextureId,
};
use graphics_base::streaming::DrawScopeImpl;
use rayon::prelude::{IndexedParallelIterator, ParallelIterator};

use base::system::SystemTimeInterface;

use math::math::vector::vec2;

use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::{ETextureIndex, GL_SColor, WriteVertexAttributes},
    types::GraphicsMemoryAllocationType,
    types::ImageFormat,
};

use super::types::{UIPipe, UIState};

/**
 * UI is not a client component, it should be cleanly separated from any game logic (but can read it)
 */
pub struct UI {
    egui_ctx: egui::Context,
    textures: HashMap<TextureId, ETextureIndex>,

    draw_ranges: Vec<(usize, usize, usize, TextureId, egui::Rect)>,
    mesh_index_offsets: Vec<usize>,

    pub ui_state: UIState,

    pub main_panel_color: Color32,
}

impl UI {
    pub fn new(zoom_level: f32) -> Self {
        let res = Self {
            egui_ctx: egui::Context::default(),

            textures: HashMap::new(),

            ui_state: UIState::new(zoom_level),

            draw_ranges: Vec::new(),
            mesh_index_offsets: Vec::new(),

            main_panel_color: Color32::TRANSPARENT,
        };
        let vis = egui::style::Visuals::dark();
        res.egui_ctx.set_visuals(vis);
        res
    }

    pub fn set_main_panel_color(&mut self, main_panel_color: &Color32) {
        self.main_panel_color = *main_panel_color;
    }

    fn main_panel(main_panel_color: &Color32) -> egui::CentralPanel {
        let standard_frame = egui::containers::Frame {
            inner_margin: egui::style::Margin {
                left: 0.,
                right: 0.,
                top: 0.,
                bottom: 0.,
            },
            outer_margin: egui::style::Margin {
                left: 0.,
                right: 0.,
                top: 0.,
                bottom: 0.,
            },
            rounding: egui::Rounding {
                nw: 0.0,
                ne: 0.0,
                sw: 0.0,
                se: 0.0,
            },
            shadow: egui::epaint::Shadow::NONE,
            fill: *main_panel_color,
            stroke: egui::Stroke::NONE,
        };
        egui::CentralPanel::default().frame(standard_frame)
    }

    pub fn render(
        &mut self,
        render_func: impl FnOnce(&mut egui::Ui, &mut UIPipe, &mut UIState),
        pipe: &mut UIPipe,
    ) {
        let canvas_width = pipe.graphics.canvas_width();
        let canvas_height = pipe.graphics.canvas_height();

        // Gather input (mouse, touches, keyboard, screen size, etc):
        let mut raw_input: egui::RawInput = egui::RawInput::default();
        let screen_rect = egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 0.0 },
            max: egui::Pos2 {
                x: canvas_width as f32 / self.ui_state.zoom_level,
                y: canvas_height as f32 / self.ui_state.zoom_level,
            },
        };
        raw_input.screen_rect = Some(screen_rect);
        raw_input.pixels_per_point = Some(self.ui_state.zoom_level);
        let cur_time_secs = pipe.sys.time_get_nanoseconds().as_nanos() as f64
            / (Duration::from_secs(1).as_nanos() as f64);
        raw_input.time = Some(cur_time_secs);
        std::mem::swap(
            &mut raw_input.events,
            &mut self.ui_state.sdl2_state.raw_input.events,
        );

        let full_output = self.egui_ctx.run(raw_input, |egui_ctx| {
            Self::main_panel(&self.main_panel_color)
                .show(egui_ctx, |ui| render_func(ui, pipe, &mut self.ui_state));
        });

        full_output
            .textures_delta
            .set
            .iter()
            .for_each(|(texture_id, delta)| {
                let tex = self.textures.get(texture_id);
                match tex {
                    // update existing texture
                    Some(_) => todo!(),
                    // create new texture
                    None => {
                        let mut tex_index = ETextureIndex::Invalid;
                        match &delta.image {
                            ImageData::Color(img) => {
                                let mut pixels = pipe.graphics.mem_alloc(
                                    GraphicsMemoryAllocationType::Texture,
                                    img.pixels.len() * 4,
                                );
                                pixels
                                    .mem()
                                    .iter_mut()
                                    .enumerate()
                                    .for_each(|(index, pixel)| {
                                        *pixel = img.pixels[index / 4].to_array()[index % 4];
                                    });
                                pixels.exec().load_texture(
                                    &mut tex_index,
                                    img.width(),
                                    img.height(),
                                    ImageFormat::Rgba as i32,
                                    TexFormat::RGBA as i32,
                                    TexFlags::TEXFLAG_NOMIPMAPS,
                                    "ui",
                                );
                            }
                            ImageData::Font(img_font) => {
                                let mut pixels_mem = pipe.graphics.mem_alloc(
                                    GraphicsMemoryAllocationType::Texture,
                                    img_font.pixels.len() * 4,
                                );
                                let pixels = pixels_mem.mem();
                                img_font.srgba_pixels(None).enumerate().for_each(
                                    |(index, img_pixel)| {
                                        let texel = img_pixel.to_array();
                                        pixels[(index * 4) + 0] = texel[0];
                                        pixels[(index * 4) + 1] = texel[1];
                                        pixels[(index * 4) + 2] = texel[2];
                                        pixels[(index * 4) + 3] = texel[3];
                                    },
                                );
                                pixels_mem.exec().load_texture(
                                    &mut tex_index,
                                    img_font.width(),
                                    img_font.height(),
                                    ImageFormat::Rgba as i32,
                                    TexFormat::RGBA as i32,
                                    TexFlags::TEXFLAG_NOMIPMAPS,
                                    "ui",
                                );
                            }
                        }
                        self.textures.insert(texture_id.clone(), tex_index);
                    }
                }
            });

        let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes); // creates triangles to paint

        let mut last_tex_index = TextureId::default();
        let mut last_clip_rect = egui::Rect::NOTHING;

        self.draw_ranges.clear();
        self.mesh_index_offsets.clear();
        let mut cur_index: usize = 0;
        let mut cur_draw_count: usize = 0;
        let mut cur_indices_count: usize = 0;

        clipped_primitives
            .iter()
            .enumerate()
            .for_each(|(index, v)| match &v.primitive {
                Primitive::Mesh(mesh) => {
                    if mesh.texture_id != last_tex_index || v.clip_rect != last_clip_rect {
                        if cur_draw_count > 0 {
                            self.draw_ranges.push((
                                cur_index,
                                cur_index + cur_draw_count,
                                cur_indices_count,
                                last_tex_index,
                                last_clip_rect,
                            ));
                        }
                        cur_index = index;
                        cur_draw_count = 0;
                        cur_indices_count = 0;
                        last_tex_index = mesh.texture_id;
                        last_clip_rect = v.clip_rect;
                    }
                    cur_draw_count += 1;
                    self.mesh_index_offsets.push(cur_indices_count);
                    cur_indices_count += mesh.indices.len();
                }
                _ => todo!(),
            });
        if cur_draw_count > 0 {
            self.draw_ranges.push((
                cur_index,
                cur_index + cur_draw_count,
                cur_indices_count,
                last_tex_index,
                last_clip_rect,
            ));
        }

        self.draw_ranges.iter().for_each(
            |(start_index, end_index, indices_count, texture_id, clip_rect)| {
                let mut draw_triangles = pipe.graphics.backend_handle.triangles_begin();
                draw_triangles.map_canvas(
                    0.0,
                    0.0,
                    canvas_width as f32 / self.ui_state.zoom_level,
                    canvas_height as f32 / self.ui_state.zoom_level,
                );
                draw_triangles.clip(
                    (clip_rect.left_top().x * self.ui_state.zoom_level) as i32,
                    ((screen_rect.height() - (clip_rect.left_top().y + clip_rect.height()))
                        * self.ui_state.zoom_level) as i32,
                    (clip_rect.width() * self.ui_state.zoom_level) as u32,
                    (clip_rect.height() * self.ui_state.zoom_level) as u32,
                );
                draw_triangles.blend_additive();
                let tex_index = self.textures.get(&texture_id);
                if let Some(tex_index) = tex_index {
                    draw_triangles.set_texture(*tex_index);
                    draw_triangles.wrap_clamp();
                }

                (*start_index..*end_index)
                    .into_iter()
                    .for_each(|prim_index| {
                        let index_offset = self.mesh_index_offsets[prim_index];
                        let index_offset_next = if prim_index + 1 < *end_index {
                            self.mesh_index_offsets
                                .get(prim_index + 1)
                                .unwrap_or(indices_count)
                        } else {
                            indices_count
                        };
                        let mut remaining_indices = index_offset_next - index_offset;
                        let mut vertex_offset: usize = 0;

                        while remaining_indices > 0 {
                            let vertices = draw_triangles.get_raw_handle(remaining_indices / 3);

                            match &clipped_primitives[prim_index].primitive {
                                Primitive::Mesh(mesh) => vertices.iter_mut().enumerate().for_each(
                                    |(vert_index, vertex)| {
                                        let index = vert_index + vertex_offset;
                                        let mesh_index = mesh.indices[index];
                                        vertex.set_pos(&vec2 {
                                            x: mesh.vertices[mesh_index as usize].pos.x,
                                            y: mesh.vertices[mesh_index as usize].pos.y,
                                        });
                                        let vert_color =
                                            mesh.vertices[mesh_index as usize].color.to_array();
                                        let color = GL_SColor {
                                            x: vert_color[0],
                                            y: vert_color[1],
                                            z: vert_color[2],
                                            w: vert_color[3],
                                        };
                                        vertex.set_color(&color);

                                        let tex = vec2 {
                                            x: mesh.vertices[mesh_index as usize].uv.x,
                                            y: mesh.vertices[mesh_index as usize].uv.y,
                                        };
                                        vertex.set_tex_coords(&tex);
                                    },
                                ),
                                _ => todo!(),
                            }

                            vertex_offset += vertices.len();
                            remaining_indices -= vertices.len();
                        }
                    });
            },
        );

        // we delete textures now, so any kind of drawing has to be finished
        full_output.textures_delta.free.iter().for_each(|tex_id| {
            let tex = self.textures.get_mut(tex_id);
            if let Some(tex) = tex {
                pipe.graphics.unload_texture(tex);

                self.textures.remove(tex_id);
            }
        });

        self.ui_state
            .sdl2_state
            .process_output(pipe.graphics.borrow_window(), &full_output.platform_output);
    }
}
