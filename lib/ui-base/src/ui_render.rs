use anyhow::anyhow;
use egui::{epaint::Primitive, FullOutput, ImageData};

use crate::ui::{UiContainer, UiContext};
use graphics::handles::{
    backend::backend::GraphicsBackendHandle,
    stream::stream::{GraphicsStreamHandle, TriangleStreamHandle},
    texture::texture::GraphicsTextureHandle,
};
use graphics_types::{
    commands::{TexFlags, TexFormat},
    rendering::{BlendType, ColorMaskMode, GlColor, SVertex, State, StencilMode, WrapType},
    types::{GraphicsMemoryAllocationType, ImageFormat},
};
use hiarc::hi_closure;
use math::math::vector::vec2;

fn render_ui_impl(
    ui: &mut UiContainer,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    backend_handle: &GraphicsBackendHandle,
    texture_handle: &GraphicsTextureHandle,
    stream_handle: &GraphicsStreamHandle,
    as_stencil: bool,
) -> egui::PlatformOutput {
    let context = if as_stencil {
        &mut ui.stencil_context
    } else {
        &mut ui.context
    };
    let mut custom_paints = std::mem::take(&mut ui.ui_state.custom_paints);

    if std::mem::take(&mut ui.should_tesselate) {
        // creates triangles to paint
        ui.last_clipped_primitives = context
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
    };
    let clipped_primitives = &ui.last_clipped_primitives;

    full_output
        .textures_delta
        .set
        .iter()
        .for_each(|(texture_id, delta)| {
            if delta.pos.is_none() {
                // pos of none basically means delete the current image and recreate it
                context.textures.borrow_mut().remove(texture_id);
            }
            let textures = context.textures.borrow();
            let tex = textures.get(texture_id);
            match tex {
                // update existing texture
                Some(tex_index) => {
                    let pos = delta.pos.unwrap_or_default();
                    match &delta.image {
                        ImageData::Color(img) => {
                            let mut pixels = Vec::<u8>::new();
                            pixels.resize(img.width() * img.height() * 4, Default::default());
                            pixels.iter_mut().enumerate().for_each(|(index, pixel)| {
                                *pixel = img.pixels[index / 4].to_array()[index % 4];
                            });
                            tex_index
                                .update_texture(
                                    pos[0] as isize,
                                    pos[1] as isize,
                                    img.width(),
                                    img.height(),
                                    pixels,
                                )
                                .unwrap();
                        }
                        ImageData::Font(img_font) => {
                            let mut pixels = Vec::<u8>::new();
                            pixels.resize(
                                img_font.width() * img_font.height() * 4,
                                Default::default(),
                            );
                            img_font.srgba_pixels(None).enumerate().for_each(
                                |(index, img_pixel)| {
                                    let texel = img_pixel.to_array();
                                    pixels.as_mut_slice()[index * 4] = texel[0];
                                    pixels.as_mut_slice()[(index * 4) + 1] = texel[1];
                                    pixels.as_mut_slice()[(index * 4) + 2] = texel[2];
                                    pixels.as_mut_slice()[(index * 4) + 3] = texel[3];
                                },
                            );
                            tex_index
                                .update_texture(
                                    pos[0] as isize,
                                    pos[1] as isize,
                                    img_font.width(),
                                    img_font.height(),
                                    pixels,
                                )
                                .unwrap();
                        }
                    }
                }
                // create new texture
                None => {
                    assert!(delta.pos.is_none(), "can this happen?");
                    let tex_index;
                    match &delta.image {
                        ImageData::Color(img) => {
                            let mut pixels =
                                backend_handle.mem_alloc(GraphicsMemoryAllocationType::Texture {
                                    width: img.width(),
                                    height: img.height(),
                                    depth: 1,
                                    is_3d_tex: false,
                                    flags: TexFlags::TEXFLAG_NOMIPMAPS,
                                });
                            pixels.as_mut_slice().iter_mut().enumerate().for_each(
                                |(index, pixel)| {
                                    *pixel = img.pixels[index / 4].to_array()[index % 4];
                                },
                            );
                            tex_index = Some(
                                texture_handle
                                    .load_texture(
                                        img.width(),
                                        img.height(),
                                        ImageFormat::Rgba,
                                        pixels,
                                        TexFormat::Rgba,
                                        TexFlags::TEXFLAG_NOMIPMAPS,
                                        "ui",
                                    )
                                    .unwrap(),
                            );
                        }
                        ImageData::Font(img_font) => {
                            let mut pixels_mem =
                                backend_handle.mem_alloc(GraphicsMemoryAllocationType::Texture {
                                    width: img_font.width(),
                                    height: img_font.height(),
                                    depth: 1,
                                    is_3d_tex: false,
                                    flags: TexFlags::TEXFLAG_NOMIPMAPS,
                                });
                            let pixels = pixels_mem.as_mut_slice();
                            img_font.srgba_pixels(None).enumerate().for_each(
                                |(index, img_pixel)| {
                                    let texel = img_pixel.to_array();
                                    pixels[index * 4] = texel[0];
                                    pixels[(index * 4) + 1] = texel[1];
                                    pixels[(index * 4) + 2] = texel[2];
                                    pixels[(index * 4) + 3] = texel[3];
                                },
                            );
                            tex_index = Some(
                                texture_handle
                                    .load_texture(
                                        img_font.width(),
                                        img_font.height(),
                                        ImageFormat::Rgba,
                                        pixels_mem,
                                        TexFormat::Rgba,
                                        TexFlags::TEXFLAG_NOMIPMAPS,
                                        "ui",
                                    )
                                    .unwrap(),
                            );
                        }
                    }
                    if let Some(tex) = tex_index {
                        drop(textures);
                        context.textures.borrow_mut().insert(*texture_id, tex);
                    }
                }
            }
        });

    clipped_primitives.iter().for_each(|v| match &v.primitive {
        Primitive::Mesh(mesh) => {
            let mut state= State::new();
            state.set_stencil_mode(if as_stencil {
                StencilMode::FillStencil
            } else {
                StencilMode::None
            });
            state.set_color_mask(if as_stencil {
                ColorMaskMode::WriteAlphaOnly
            } else {
                ColorMaskMode::WriteAll
            });
            state.map_canvas(
                screen_rect.left_top().x,
                screen_rect.left_top().y,
                screen_rect.width(),
                screen_rect.height(),
            );

            state.clip_auto_rounding(
                v.clip_rect.left_top().x * zoom_level,
                v.clip_rect.left_top().y * zoom_level,
                v.clip_rect.width() * zoom_level,
                v.clip_rect.height() * zoom_level,
            );

            state.blend(BlendType::Additive);
            state.wrap(WrapType::Clamp);
            stream_handle.render_triangles(hi_closure!([context: &mut UiContext, mesh: &egui::Mesh], |mut stream_handle: TriangleStreamHandle<'_>| -> () {
                let textures = context.textures.borrow();
                let tex_index = textures.get(&mesh.texture_id);
                if let Some(tex_index) = tex_index {
                    stream_handle.set_texture(tex_index);
                }

                for vert_index in 0..mesh.indices.len() / 3 {
                    let mut vertices: [SVertex; 3] = Default::default();
                    for (i, vertex) in vertices.iter_mut().enumerate() {
                        let index = vert_index;
                        let mesh_index = mesh.indices[index * 3  + i];
                        vertex.set_pos(&vec2 {
                            x: mesh.vertices[mesh_index as usize].pos.x,
                            y: mesh.vertices[mesh_index as usize].pos.y,
                        });
                        let vert_color = mesh.vertices[mesh_index as usize].color.to_array();
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
                    }
                    stream_handle.add_vertices(vertices);
                }
            }), state);

        }
        Primitive::Callback(cb) => {
            // TODO: support custom pipes?
            let cb = cb.callback.downcast_ref::<u64>().ok_or_else(|| anyhow!("Custom callback must be u64 and added over `UiState`")).unwrap();

            if let Some(custom_paint) = custom_paints.remove(cb) {
                custom_paint.render();
            }
        }
    });

    // we delete textures now, so any kind of drawing has to have finished
    full_output.textures_delta.free.iter().for_each(|tex_id| {
        let _ = context.textures.borrow_mut().remove(tex_id);
    });

    full_output.platform_output
}

pub fn render_ui(
    ui: &mut UiContainer,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    backend_handle: &GraphicsBackendHandle,
    texture_handle: &GraphicsTextureHandle,
    stream_handle: &GraphicsStreamHandle,
    as_stencil: bool,
) -> egui::PlatformOutput {
    render_ui_impl(
        ui,
        full_output,
        screen_rect,
        zoom_level,
        backend_handle,
        texture_handle,
        stream_handle,
        as_stencil,
    )
}
