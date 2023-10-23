use anyhow::anyhow;
use egui::{epaint::Primitive, FullOutput, ImageData};

use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_base::streaming::{
    DrawScopeImpl, DrawStreamImplSimplified, GraphicsStreamHandleInterface,
};
use graphics_types::{
    command_buffer::{TexFlags, TexFormat},
    rendering::{GlColor, RenderMode, WriteVertexAttributes},
    types::{GraphicsMemoryAllocationType, ImageFormat},
};
use math::math::vector::vec2;

use crate::{custom_callback::CustomCallback, types::UINativePipe, ui::UI};

pub fn render_ui_impl<T, B: Clone, C1: 'static, C2: 'static, C3: 'static>(
    ui: &mut UI<T>,
    native_pipe: &mut UINativePipe<T>,
    custom_callback_type1: &mut C1,
    custom_callback_type2: &mut C2,
    custom_callback_type3: &mut C3,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    graphics: &mut GraphicsBase<B>,
    as_stencil: bool,
) where
    B: GraphicsBackendInterface + 'static,
{
    let context = if as_stencil {
        &mut ui.stencil_context
    } else {
        &mut ui.context
    };
    let clipped_primitives = context.egui_ctx.tessellate(full_output.shapes); // creates triangles to paint

    full_output
        .textures_delta
        .set
        .iter()
        .for_each(|(texture_id, delta)| {
            let tex = context.textures.get(texture_id);
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
                            graphics
                                .texture_handle
                                .update_texture(
                                    tex_index,
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
                                    pixels.as_mut_slice()[(index * 4) + 0] = texel[0];
                                    pixels.as_mut_slice()[(index * 4) + 1] = texel[1];
                                    pixels.as_mut_slice()[(index * 4) + 2] = texel[2];
                                    pixels.as_mut_slice()[(index * 4) + 3] = texel[3];
                                },
                            );
                            graphics
                                .texture_handle
                                .update_texture(
                                    tex_index,
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
                                graphics.mem_alloc(GraphicsMemoryAllocationType::Texture {
                                    width: img.width(),
                                    height: img.height(),
                                    depth: 1,
                                    is_3d_tex: false,
                                    flags: TexFlags::TEXFLAG_NOMIPMAPS,
                                });
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
                            let mut pixels_mem =
                                graphics.mem_alloc(GraphicsMemoryAllocationType::Texture {
                                    width: img_font.width(),
                                    height: img_font.height(),
                                    depth: 1,
                                    is_3d_tex: false,
                                    flags: TexFlags::TEXFLAG_NOMIPMAPS,
                                });
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
                        context.textures.insert(texture_id.clone(), tex);
                    }
                }
            }
        });

    clipped_primitives.iter().for_each(|v| match &v.primitive {
        Primitive::Mesh(mesh) => {
            let mut draw_triangles = graphics.stream_handle.triangles_begin();
            draw_triangles.set_render_mode(if as_stencil {
                RenderMode::FillStencil
            } else {
                RenderMode::Standard
            });
            draw_triangles.map_canvas(
                screen_rect.left_top().x,
                screen_rect.left_top().y,
                screen_rect.width(),
                screen_rect.height(),
            );

            draw_triangles.clip_auto_rounding(
                v.clip_rect.left_top().x * zoom_level,
                (screen_rect.height() - (v.clip_rect.left_top().y + v.clip_rect.height()))
                    * zoom_level,
                v.clip_rect.width() * zoom_level,
                v.clip_rect.height() * zoom_level,
            );

            draw_triangles.blend_additive();
            let tex_index = context.textures.get(&mesh.texture_id);
            if let Some(tex_index) = tex_index {
                draw_triangles.set_texture(tex_index);
                draw_triangles.wrap_clamp();
            }

            let mut vertices = draw_triangles.get_raw_handle(mesh.indices.len() / 3);

            vertices
                .get()
                .iter_mut()
                .enumerate()
                .for_each(|(vert_index, vertex)| {
                    let index = vert_index;
                    let mesh_index = mesh.indices[index];
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
                });
        }
        Primitive::Callback(cb) => {
            // TODO: support custom pipes?
            let cb = cb.callback.downcast_ref::<CustomCallback<B, C1, C2, C3>>().ok_or_else(|| anyhow!("Custom callback downcasting failed. Did you use the appropriate CustomCallbackPipeline template types?")).unwrap();

            match cb.custom_type_count {
                1 => cb.cb.render1(graphics, custom_callback_type1),
                2 => cb.cb.render2(graphics, custom_callback_type1, custom_callback_type2),
                3 => cb.cb.render3(graphics, custom_callback_type1, custom_callback_type2, custom_callback_type3),
                _ => panic!("this amount of render custom types are not supported ({})", cb.custom_type_count)
            }
        }
    });

    // we delete textures now, so any kind of drawing has to be finished
    full_output.textures_delta.free.iter().for_each(|tex_id| {
        let _ = context.textures.remove(tex_id);
    });

    //ui.ui_state
    native_pipe.raw_inp_generator.process_output(
        &mut ui.ui_native_state,
        &context.egui_ctx,
        full_output.platform_output,
    );
}

pub fn render_ui<T, B: Clone>(
    ui: &mut UI<T>,
    native_pipe: &mut UINativePipe<T>,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    graphics: &mut GraphicsBase<B>,
    as_stencil: bool,
) where
    B: GraphicsBackendInterface + 'static,
{
    render_ui_impl(
        ui,
        native_pipe,
        &mut (),
        &mut (),
        &mut (),
        full_output,
        screen_rect,
        zoom_level,
        graphics,
        as_stencil,
    )
}

pub fn render_ui_1<T, B: Clone, C1: 'static>(
    ui: &mut UI<T>,
    native_pipe: &mut UINativePipe<T>,
    custom_callback_type1: &mut C1,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    graphics: &mut GraphicsBase<B>,
    as_stencil: bool,
) where
    B: GraphicsBackendInterface + 'static,
{
    render_ui_impl(
        ui,
        native_pipe,
        custom_callback_type1,
        &mut (),
        &mut (),
        full_output,
        screen_rect,
        zoom_level,
        graphics,
        as_stencil,
    )
}

pub fn render_ui_2<T, B: Clone, C1: 'static, C2: 'static>(
    ui: &mut UI<T>,
    native_pipe: &mut UINativePipe<T>,
    custom_callback_type1: &mut C1,
    custom_callback_type2: &mut C2,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    graphics: &mut GraphicsBase<B>,
    as_stencil: bool,
) where
    B: GraphicsBackendInterface + 'static,
{
    render_ui_impl(
        ui,
        native_pipe,
        custom_callback_type1,
        custom_callback_type2,
        &mut (),
        full_output,
        screen_rect,
        zoom_level,
        graphics,
        as_stencil,
    )
}
