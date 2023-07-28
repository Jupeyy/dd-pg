use egui::{epaint::Primitive, FullOutput, ImageData, TextureId};
use graphics::graphics::Graphics;
use graphics_base::streaming::DrawScopeImpl;
use graphics_render_traits::GraphicsRenderGeometry;
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::{GlColor, WriteVertexAttributes},
    types::{GraphicsMemoryAllocationType, ImageFormat},
};
use math::math::vector::vec2;
use ui_base::ui::UI;

pub fn render_ui<T>(
    ui: &mut UI<T>,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    graphics: &mut Graphics,
) {
    let clipped_primitives = ui.egui_ctx.tessellate(full_output.shapes); // creates triangles to paint

    full_output
        .textures_delta
        .set
        .iter()
        .for_each(|(texture_id, delta)| {
            let tex = ui.textures.get(texture_id);
            match tex {
                // update existing texture
                Some(_) => todo!(),
                // create new texture
                None => {
                    let tex_index;
                    match &delta.image {
                        ImageData::Color(img) => {
                            let mut pixels = graphics.mem_alloc(
                                GraphicsMemoryAllocationType::Texture,
                                img.pixels.len() * 4,
                            );
                            pixels.mem().as_mut_slice().iter_mut().enumerate().for_each(
                                |(index, pixel)| {
                                    *pixel = img.pixels[index / 4].to_array()[index % 4];
                                },
                            );
                            tex_index = Some(
                                pixels
                                    .exec()
                                    .load_texture(
                                        img.width(),
                                        img.height(),
                                        ImageFormat::Rgba as i32,
                                        TexFormat::RGBA as i32,
                                        TexFlags::TEXFLAG_NOMIPMAPS,
                                        "ui",
                                    )
                                    .unwrap(),
                            );
                        }
                        ImageData::Font(img_font) => {
                            let mut pixels_mem = graphics.mem_alloc(
                                GraphicsMemoryAllocationType::Texture,
                                img_font.pixels.len() * 4,
                            );
                            let pixels = pixels_mem.mem();
                            img_font.srgba_pixels(None).enumerate().for_each(
                                |(index, img_pixel)| {
                                    let texel = img_pixel.to_array();
                                    pixels.as_mut_slice()[(index * 4) + 0] = texel[0];
                                    pixels.as_mut_slice()[(index * 4) + 1] = texel[1];
                                    pixels.as_mut_slice()[(index * 4) + 2] = texel[2];
                                    pixels.as_mut_slice()[(index * 4) + 3] = texel[3];
                                },
                            );
                            tex_index = Some(
                                pixels_mem
                                    .exec()
                                    .load_texture(
                                        img_font.width(),
                                        img_font.height(),
                                        ImageFormat::Rgba as i32,
                                        TexFormat::RGBA as i32,
                                        TexFlags::TEXFLAG_NOMIPMAPS,
                                        "ui",
                                    )
                                    .unwrap(),
                            );
                        }
                    }
                    if let Some(tex) = tex_index {
                        ui.textures.insert(texture_id.clone(), tex);
                    }
                }
            }
        });

    let mut last_tex_index = TextureId::default();
    let mut last_clip_rect = egui::Rect::NOTHING;

    ui.draw_ranges.clear();
    ui.mesh_index_offsets.clear();
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
                        ui.draw_ranges.push((
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
                ui.mesh_index_offsets.push(cur_indices_count);
                cur_indices_count += mesh.indices.len();
            }
            _ => todo!(),
        });
    if cur_draw_count > 0 {
        ui.draw_ranges.push((
            cur_index,
            cur_index + cur_draw_count,
            cur_indices_count,
            last_tex_index,
            last_clip_rect,
        ));
    }

    ui.draw_ranges.iter().for_each(
        |(start_index, end_index, indices_count, texture_id, clip_rect)| {
            let mut draw_triangles = graphics.backend_handle.triangles_begin();
            draw_triangles.map_canvas(
                screen_rect.left_top().x,
                screen_rect.left_top().y,
                screen_rect.width(),
                screen_rect.height(),
            );
            draw_triangles.clip(
                (clip_rect.left_top().x * ui.ui_state.zoom_level) as i32,
                ((screen_rect.height() - (clip_rect.left_top().y + clip_rect.height()))
                    * ui.ui_state.zoom_level) as i32,
                (clip_rect.width() * ui.ui_state.zoom_level) as u32,
                (clip_rect.height() * ui.ui_state.zoom_level) as u32,
            );
            draw_triangles.blend_additive();
            let tex_index = ui.textures.get(&texture_id);
            if let Some(tex_index) = tex_index {
                draw_triangles.set_texture(tex_index);
                draw_triangles.wrap_clamp();
            }

            (*start_index..*end_index)
                .into_iter()
                .for_each(|prim_index| {
                    let index_offset = ui.mesh_index_offsets[prim_index];
                    let index_offset_next = if prim_index + 1 < *end_index {
                        ui.mesh_index_offsets
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
                            Primitive::Mesh(mesh) => {
                                vertices
                                    .iter_mut()
                                    .enumerate()
                                    .for_each(|(vert_index, vertex)| {
                                        let index = vert_index + vertex_offset;
                                        let mesh_index = mesh.indices[index];
                                        vertex.set_pos(&vec2 {
                                            x: mesh.vertices[mesh_index as usize].pos.x,
                                            y: mesh.vertices[mesh_index as usize].pos.y,
                                        });
                                        let vert_color =
                                            mesh.vertices[mesh_index as usize].color.to_array();
                                        let color = GlColor {
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
                                    })
                            }
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
        let tex = ui.textures.remove(tex_id);
        if let Some(tex) = tex {
            graphics.unload_texture(tex);
        }
    });

    //ui.ui_state
    /*process_output(
        &mut ui.ui_state.sdl2_state,
        graphics.borrow_window(),
        &full_output.platform_output,
    );*/
}

pub fn destroy_ui<T>(mut ui: UI<T>, graphics: &mut Graphics) {
    for (_, texture_id) in ui.textures.drain() {
        graphics.unload_texture(texture_id);
    }
}
