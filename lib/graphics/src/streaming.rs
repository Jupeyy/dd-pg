use std::{cell::RefMut, ops::Range};

use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{
    commands::{SColor, STexCoord},
    rendering::{
        BlendType, ColorMaskMode, GlPoint, GlVertex, RenderMode, SVertex, State, StencilMode,
        WrapType, WriteVertexAttributes,
    },
    textures_handle::TextureIndex,
    types::{DrawModes, Line, StreamedQuad, Triangle},
};
use math::math::vector::{vec2, vec4};

use crate::handles::stream::GraphicsStreamHandle;

pub fn quads_draw_tl_impl<T>(
    quad_info: &mut DrawScope<4>,
    stream_info: &mut DrawStream<4>,
    quads: &[StreamedQuad],
) where
    T: WriteVertexAttributes,
{
    let stream_handle = &mut stream_info.stream_handle;
    let mut stream_data = stream_handle.stream_data().borrow_mut();
    let (vertices, vertices_count) = stream_data.vertices_and_count_mut();
    for i in 0..quads.len() {
        let index = *vertices_count;
        vertices[index as usize + 4 * i + 0].set_pos(&GlPoint {
            x: quads[i].x0,
            y: quads[i].y0,
        });
        vertices[index as usize + 4 * i + 0].set_tex_coords(&quad_info.texture_coords[0]);
        vertices[index as usize + 4 * i + 0].set_color(&quad_info.colors[0]);

        vertices[index as usize + 4 * i + 1].set_pos(&GlPoint {
            x: quads[i].x1,
            y: quads[i].y1,
        });
        vertices[index as usize + 4 * i + 1].set_tex_coords(&quad_info.texture_coords[1]);
        vertices[index as usize + 4 * i + 1].set_color(&quad_info.colors[1]);

        vertices[index as usize + 4 * i + 2].set_pos(&GlPoint {
            x: quads[i].x2,
            y: quads[i].y2,
        });
        vertices[index as usize + 4 * i + 2].set_tex_coords(&quad_info.texture_coords[2]);
        vertices[index as usize + 4 * i + 2].set_color(&quad_info.colors[2]);

        vertices[index as usize + 4 * i + 3].set_pos(&GlPoint {
            x: quads[i].x3,
            y: quads[i].y3,
        });
        vertices[index as usize + 4 * i + 3].set_tex_coords(&quad_info.texture_coords[3]);
        vertices[index as usize + 4 * i + 3].set_color(&quad_info.colors[3]);

        if quad_info.rotation != 0.0 {
            let mut center = vec2::default();
            center.x = quads[i].x() + quads[i].width() / 2.0;
            center.y = quads[i].y() + quads[i].height() / 2.0;

            rotate(
                &center,
                quad_info.rotation,
                &mut vertices[index as usize + 4 * i..(index as usize + 4 * i) + 4],
            );
        }

        *vertices_count += 4;
    }
}

pub fn triangle_draw_tl_impl<T>(
    tri_info: &mut DrawScope<3>,
    stream_info: &mut DrawStream<3>,
    triangles: &[Triangle],
) where
    T: WriteVertexAttributes,
{
    let stream_handle = &mut stream_info.stream_handle;
    let mut stream_data = stream_handle.stream_data().borrow_mut();
    let (vertices, vertices_count) = stream_data.vertices_and_count_mut();
    for i in 0..triangles.len() {
        let index = *vertices_count;
        vertices[index as usize + 3 * i + 0].set_pos(&GlPoint {
            x: triangles[i].vertices[0].x,
            y: triangles[i].vertices[0].y,
        });
        vertices[index as usize + 3 * i + 0].set_tex_coords(&tri_info.texture_coords[0]);
        vertices[index as usize + 3 * i + 0].set_color(&tri_info.colors[0]);

        vertices[index as usize + 3 * i + 1].set_pos(&GlPoint {
            x: triangles[i].vertices[1].x,
            y: triangles[i].vertices[1].y,
        });
        vertices[index as usize + 3 * i + 1].set_tex_coords(&tri_info.texture_coords[1]);
        vertices[index as usize + 3 * i + 1].set_color(&tri_info.colors[1]);

        vertices[index as usize + 3 * i + 2].set_pos(&GlPoint {
            x: triangles[i].vertices[2].x,
            y: triangles[i].vertices[2].y,
        });
        vertices[index as usize + 3 * i + 2].set_tex_coords(&tri_info.texture_coords[2]);
        vertices[index as usize + 3 * i + 2].set_color(&tri_info.colors[2]);

        *vertices_count += 3;
    }
}

pub fn lines_draw_tl_impl<T>(stream_info: &mut DrawStream<2>, lines: &[Line])
where
    T: WriteVertexAttributes,
{
    let stream_handle = &mut stream_info.stream_handle;
    let mut stream_data = stream_handle.stream_data().borrow_mut();
    let (vertices, vertices_count) = stream_data.vertices_and_count_mut();
    for i in 0..lines.len() {
        let index = *vertices_count;
        vertices[index as usize + 2 * i + 0].set_pos(&GlPoint {
            x: lines[i].vertices[0].x,
            y: lines[i].vertices[0].y,
        });
        vertices[index as usize + 2 * i + 1].set_pos(&GlPoint {
            x: lines[i].vertices[1].x,
            y: lines[i].vertices[1].y,
        });

        *vertices_count += 2;
    }
}

pub fn rotate<T>(center: &vec2, rotation: f32, points: &mut [T])
where
    T: WriteVertexAttributes,
{
    let c = rotation.cos();
    let s = rotation.sin();

    for i in 0..points.len() {
        let x = points[i].get_pos().x - center.x;
        let y = points[i].get_pos().y - center.y;
        points[i].set_pos(&vec2 {
            x: x * c - y * s + center.x,
            y: x * s + y * c + center.y,
        });
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrawScope<const VERTEX_COUNT: usize> {
    pub state: State,
    pub render_mode: RenderMode,
    pub tile_index: u8,
    pub rotation: f32,
    pub colors: [SColor; VERTEX_COUNT],
    pub texture_coords: [STexCoord; VERTEX_COUNT],
}

impl<const VERTEX_COUNT: usize> DrawScope<VERTEX_COUNT> {
    pub fn new() -> Self {
        Self {
            state: State::new(),
            render_mode: RenderMode::default(),
            colors: [(); { VERTEX_COUNT }].map(|_| SColor::default()),
            texture_coords: [(); { VERTEX_COUNT }].map(|_| STexCoord::default()),
            tile_index: Default::default(),
            rotation: Default::default(),
        }
    }

    pub fn set_colors_from_single(&mut self, r: f32, g: f32, b: f32, a: f32) {
        let red = r.clamp(0.0, 1.0) * 255.0;
        let green = g.clamp(0.0, 1.0) * 255.0;
        let blue = b.clamp(0.0, 1.0) * 255.0;
        let alpha = a.clamp(0.0, 1.0) * 255.0;

        for color in &mut self.colors {
            color.set_r(red as u8);
            color.set_g(green as u8);
            color.set_b(blue as u8);
            color.set_a(alpha as u8);
        }
    }

    pub fn set_colors(&mut self, colors: &[vec4; VERTEX_COUNT]) {
        for (index, color) in self.colors.iter_mut().enumerate() {
            let red = colors[index].r().clamp(0.0, 1.0) * 255.0;
            let green = colors[index].g().clamp(0.0, 1.0) * 255.0;
            let blue = colors[index].b().clamp(0.0, 1.0) * 255.0;
            let alpha = colors[index].a().clamp(0.0, 1.0) * 255.0;

            color.set_r(red as u8);
            color.set_g(green as u8);
            color.set_b(blue as u8);
            color.set_a(alpha as u8);
        }
    }

    pub fn set_state(&mut self, state: &State) {
        self.state = *state;
    }

    pub fn set_render_mode(&mut self, render_mode: RenderMode) {
        self.render_mode = render_mode;
    }

    fn set_stencil_mode(&mut self, stencil_mode: StencilMode) {
        self.state.set_stencil_mode(stencil_mode);
    }

    fn set_color_mask(&mut self, color_mask: ColorMaskMode) {
        self.state.set_color_mask(color_mask);
    }

    pub fn set_texture(&mut self, tex_index: &TextureIndex) {
        self.state.set_texture(tex_index);
    }

    pub fn set_color_attachment_texture(&mut self) {
        self.state.set_color_attachment_texture();
    }

    pub fn blend(&mut self, mode: BlendType) {
        self.state.blend(mode);
    }

    /// see `State::clip`
    pub fn clip(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.state.clip(x, y, w, h);
    }

    /// see `State::clip_auto_rounding`
    pub fn clip_auto_rounding(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.state.clip_auto_rounding(x, y, w, h);
    }

    pub fn wrap(&mut self, mode: WrapType) {
        self.state.wrap(mode);
    }

    pub fn map_canvas(
        &mut self,
        top_left_x: f32,
        top_left_y: f32,
        bottom_right_x: f32,
        bottom_right_y: f32,
    ) {
        self.state
            .map_canvas(top_left_x, top_left_y, bottom_right_x, bottom_right_y);
    }

    pub fn get_canvas_mapping(&self) -> (f32, f32, f32, f32) {
        self.state.get_canvas_mapping()
    }

    pub fn set_rotation(&mut self, angle: f32) {
        self.rotation = angle;
    }
}

pub trait DrawScopeImpl<const VERTEX_COUNT: usize> {
    fn get_draw_scope(&mut self) -> &mut DrawScope<VERTEX_COUNT>;

    fn set_colors_from_single(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.get_draw_scope().set_colors_from_single(r, g, b, a);
    }

    fn set_colors(&mut self, colors: &[vec4; VERTEX_COUNT]) {
        self.get_draw_scope().set_colors(colors);
    }

    fn set_state(&mut self, state: &State) {
        self.get_draw_scope().set_state(state);
    }

    fn set_render_mode(&mut self, render_mode: RenderMode) {
        self.get_draw_scope().set_render_mode(render_mode);
    }

    fn set_stencil_mode(&mut self, stencil_mode: StencilMode) {
        self.get_draw_scope().set_stencil_mode(stencil_mode);
    }

    fn set_color_mask(&mut self, color_mask: ColorMaskMode) {
        self.get_draw_scope().set_color_mask(color_mask);
    }

    fn set_texture(&mut self, tex_index: &TextureIndex) {
        self.get_draw_scope().set_texture(tex_index);
    }

    fn set_color_attachment_texture(&mut self) {
        self.get_draw_scope().set_color_attachment_texture();
    }

    fn wrap(&mut self, mode: WrapType) {
        self.get_draw_scope().wrap(mode);
    }

    fn blend(&mut self, mode: BlendType) {
        self.get_draw_scope().blend(mode);
    }

    /// see `State::clip`
    fn clip(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.get_draw_scope().clip(x, y, w, h);
    }

    /// see `State::clip_auto_rounding`
    fn clip_auto_rounding(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.get_draw_scope().clip_auto_rounding(x, y, w, h);
    }

    fn map_canvas(
        &mut self,
        top_left_x: f32,
        top_left_y: f32,
        bottom_right_x: f32,
        bottom_right_y: f32,
    ) {
        self.get_draw_scope()
            .map_canvas(top_left_x, top_left_y, bottom_right_x, bottom_right_y);
    }

    fn get_canvas_mapping(&mut self) -> (f32, f32, f32, f32) {
        self.get_draw_scope().get_canvas_mapping()
    }
}

pub struct RawVerticesHandle<'a> {
    handle: RefMut<'a, dyn GraphicsStreamDataInterface>,
    range: Range<usize>,
}

impl<'a> RawVerticesHandle<'a> {
    pub fn get(&mut self) -> &mut [GlVertex] {
        let range = self.range.clone();
        &mut self.handle.vertices_mut()[range]
    }
}

pub trait DrawStreamImpl<const VERTEX_COUNT: usize> {
    fn get_raw_handle(&mut self, number_of_prims: usize) -> RawVerticesHandle;
}

pub trait DrawStreamImplSimplified<const VERTEX_COUNT: usize> {
    fn get_raw_handle(&mut self, number_of_prims: usize) -> RawVerticesHandle;
}

pub struct DrawStream<'a, const VERTEX_COUNT: usize> {
    pub vertices_offset: usize,
    pub stream_handle: &'a mut GraphicsStreamHandle,
}

impl<'a, const VERTEX_COUNT: usize> DrawStream<'a, VERTEX_COUNT> {
    pub fn new(stream_handle: &'a mut GraphicsStreamHandle, vertices_offset: usize) -> Self {
        Self {
            stream_handle,
            vertices_offset,
        }
    }

    /**
     * Tries to create a slice of vertices required for of the given number of primitives
     * Might however not return that size
     */
    pub fn get_raw_handle_impl(&mut self, number_of_prims: usize) -> RawVerticesHandle {
        if number_of_prims * VERTEX_COUNT
            > self.stream_handle.stream_data().borrow().vertices().len()
                - self.stream_handle.stream_data().borrow().vertices_count()
        {
            self.stream_handle
                .flush_commands_and_reset_vertices(&mut self.vertices_offset);
        }
        let offset = self.stream_handle.stream_data().borrow().vertices_count();
        let number_of_vert = std::cmp::min(
            number_of_prims * VERTEX_COUNT,
            ((self.stream_handle.stream_data().borrow().vertices().len()
                - self.stream_handle.stream_data().borrow().vertices_count())
                / VERTEX_COUNT)
                * VERTEX_COUNT,
        );
        *self
            .stream_handle
            .stream_data()
            .borrow_mut()
            .vertices_count_mut() += number_of_vert;
        let vertices_count = self.stream_handle.stream_data().borrow().vertices_count();
        RawVerticesHandle {
            handle: self.stream_handle.stream_data().borrow_mut(),
            range: (offset..vertices_count),
        }
    }
}

impl<'a, const VERTEX_COUNT: usize> DrawStreamImpl<VERTEX_COUNT> for DrawStream<'a, VERTEX_COUNT> {
    fn get_raw_handle(&mut self, number_of_prims: usize) -> RawVerticesHandle {
        self.get_raw_handle_impl(number_of_prims)
    }
}

pub struct DrawQuads<'a> {
    pub draw_scope: DrawScope<4>,
    pub draw_stream: DrawStream<'a, 4>,
}

impl<'a> DrawQuads<'a> {
    pub fn new(stream_handle: &'a mut GraphicsStreamHandle, vertices_offset: usize) -> Self {
        Self {
            draw_scope: DrawScope::new(),
            draw_stream: DrawStream::new(stream_handle, vertices_offset),
        }
    }

    pub fn from_draw_scope(
        draw_scope: DrawScope<4>,
        stream_handle: &'a mut GraphicsStreamHandle,
        vertices_offset: usize,
    ) -> Self {
        Self {
            draw_scope,
            draw_stream: DrawStream::new(stream_handle, vertices_offset),
        }
    }

    pub fn quads_set_subset_free(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        index: u8,
    ) {
        *self.draw_scope.texture_coords[0].u() = x0;
        *self.draw_scope.texture_coords[0].v() = y0;
        *self.draw_scope.texture_coords[1].u() = x1;
        *self.draw_scope.texture_coords[1].v() = y1;
        *self.draw_scope.texture_coords[2].u() = x2;
        *self.draw_scope.texture_coords[2].v() = y2;
        *self.draw_scope.texture_coords[3].u() = x3;
        *self.draw_scope.texture_coords[3].v() = y3;
        self.draw_scope.tile_index = index;
    }

    pub fn quads_draw_tl(&mut self, quads: &[StreamedQuad]) {
        if quads.len() * 4
            > self
                .draw_stream
                .stream_handle
                .stream_data()
                .borrow()
                .vertices()
                .len()
                - self
                    .draw_stream
                    .stream_handle
                    .stream_data()
                    .borrow()
                    .vertices_count()
        {
            self.draw_stream
                .stream_handle
                .flush_commands_and_reset_vertices(&mut self.draw_stream.vertices_offset);

            if quads.len() * 4
                > self
                    .draw_stream
                    .stream_handle
                    .stream_data()
                    .borrow()
                    .vertices()
                    .len()
                    - self
                        .draw_stream
                        .stream_handle
                        .stream_data()
                        .borrow()
                        .vertices_count()
            {
                panic!("rendered too many vertices at once.");
            }
        }
        quads_draw_tl_impl::<SVertex>(&mut self.draw_scope, &mut self.draw_stream, quads);
    }
}

impl<'a> Drop for DrawQuads<'a> {
    fn drop(&mut self) {
        self.draw_stream.stream_handle.flush_vertices(
            &self.draw_scope.state,
            &self.draw_scope.render_mode,
            self.draw_stream.vertices_offset,
            DrawModes::Quads,
        );
    }
}

impl<'a> DrawScopeImpl<4> for DrawQuads<'a> {
    fn get_draw_scope(&mut self) -> &mut DrawScope<4> {
        &mut self.draw_scope
    }
}

impl<'a> DrawStreamImplSimplified<4> for DrawQuads<'a> {
    fn get_raw_handle(&mut self, number_of_prims: usize) -> RawVerticesHandle {
        self.draw_stream.get_raw_handle(number_of_prims)
    }
}

pub struct DrawTriangles<'a> {
    draw_scope: DrawScope<3>,
    draw_stream: DrawStream<'a, 3>,
}

impl<'a> DrawTriangles<'a> {
    pub fn new(stream_handle: &'a mut GraphicsStreamHandle, vertices_offset: usize) -> Self {
        Self {
            draw_scope: DrawScope::new(),
            draw_stream: DrawStream::new(stream_handle, vertices_offset),
        }
    }

    pub fn triangles_set_subset_free(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    ) {
        *self.draw_scope.texture_coords[0].u() = x0;
        *self.draw_scope.texture_coords[0].v() = y0;
        *self.draw_scope.texture_coords[1].u() = x1;
        *self.draw_scope.texture_coords[1].v() = y1;
        *self.draw_scope.texture_coords[2].u() = x2;
        *self.draw_scope.texture_coords[2].v() = y2;
    }

    pub fn triangles_draw_tl(&mut self, triangles: &[Triangle]) {
        let _vert_dummy = Vec::<GlVertex>::new();
        if triangles.len() * 3
            > self
                .draw_stream
                .stream_handle
                .stream_data()
                .borrow()
                .vertices()
                .len()
                - self
                    .draw_stream
                    .stream_handle
                    .stream_data()
                    .borrow()
                    .vertices_count()
        {
            self.draw_stream
                .stream_handle
                .flush_commands_and_reset_vertices(&mut self.draw_stream.vertices_offset);
        }
        triangle_draw_tl_impl::<SVertex>(&mut self.draw_scope, &mut self.draw_stream, triangles);
    }
}

impl<'a> Drop for DrawTriangles<'a> {
    fn drop(&mut self) {
        self.draw_stream.stream_handle.flush_vertices(
            &self.draw_scope.state,
            &self.draw_scope.render_mode,
            self.draw_stream.vertices_offset,
            DrawModes::Triangles,
        );
    }
}

impl<'a> DrawScopeImpl<3> for DrawTriangles<'a> {
    fn get_draw_scope(&mut self) -> &mut DrawScope<3> {
        &mut self.draw_scope
    }
}

impl<'a> DrawStreamImplSimplified<3> for DrawTriangles<'a> {
    fn get_raw_handle(&mut self, number_of_prims: usize) -> RawVerticesHandle {
        self.draw_stream.get_raw_handle(number_of_prims)
    }
}

pub struct DrawLines<'a> {
    draw_scope: DrawScope<2>,
    draw_stream: DrawStream<'a, 2>,
}

impl<'a> DrawLines<'a> {
    pub fn new(stream_handle: &'a mut GraphicsStreamHandle, vertices_offset: usize) -> Self {
        Self {
            draw_scope: DrawScope::new(),
            draw_stream: DrawStream::new(stream_handle, vertices_offset),
        }
    }

    pub fn from_draw_scope(
        draw_scope: DrawScope<2>,
        stream_handle: &'a mut GraphicsStreamHandle,
        vertices_offset: usize,
    ) -> Self {
        Self {
            draw_scope,
            draw_stream: DrawStream::new(stream_handle, vertices_offset),
        }
    }

    pub fn lines_draw_tl(&mut self, lines: &[Line]) {
        let _vert_dummy = Vec::<GlVertex>::new();
        if lines.len() * 2
            > self
                .draw_stream
                .stream_handle
                .stream_data()
                .borrow()
                .vertices()
                .len()
                - self
                    .draw_stream
                    .stream_handle
                    .stream_data()
                    .borrow()
                    .vertices_count()
        {
            self.draw_stream
                .stream_handle
                .flush_commands_and_reset_vertices(&mut self.draw_stream.vertices_offset);

            if lines.len() * 2
                > self
                    .draw_stream
                    .stream_handle
                    .stream_data()
                    .borrow()
                    .vertices()
                    .len()
                    - self
                        .draw_stream
                        .stream_handle
                        .stream_data()
                        .borrow()
                        .vertices_count()
            {
                panic!("rendered too many vertices at once.");
            }
        }
        lines_draw_tl_impl::<SVertex>(&mut self.draw_stream, lines);
    }
}

impl<'a> Drop for DrawLines<'a> {
    fn drop(&mut self) {
        self.draw_stream.stream_handle.flush_vertices(
            &self.draw_scope.state,
            &self.draw_scope.render_mode,
            self.draw_stream.vertices_offset,
            DrawModes::Lines,
        );
    }
}

impl<'a> DrawScopeImpl<2> for DrawLines<'a> {
    fn get_draw_scope(&mut self) -> &mut DrawScope<2> {
        &mut self.draw_scope
    }
}

impl<'a> DrawStreamImplSimplified<2> for DrawLines<'a> {
    fn get_raw_handle(&mut self, number_of_prims: usize) -> RawVerticesHandle {
        self.draw_stream.get_raw_handle(number_of_prims)
    }
}

pub fn quad_scope_begin() -> DrawScope<4> {
    DrawScope::new()
}
