use graphics_traits::GraphicsStreamHandler;
use graphics_types::{
    command_buffer::{SColor, STexCoord},
    rendering::{ETextureIndex, GL_SPoint, GL_SVertex, SVertex, State, WriteVertexAttributes},
    types::{CQuadItem, DrawModes, Line, Triangle},
};
use math::math::vector::{vec2, vec4};

pub fn quads_draw_tl_impl<T>(quad_info: &mut DrawScope<4>, quads: &[CQuadItem])
where
    T: WriteVertexAttributes,
{
    let backend_handle = quad_info.backend_handle.backend_buffer_mut();
    let (vertices, vertices_count) = backend_handle.vertices_and_count_mut();
    for i in 0..quads.len() {
        let index = *vertices_count;
        vertices[index as usize + 4 * i + 0].set_pos(&GL_SPoint {
            x: quads[i].x,
            y: quads[i].y,
        });
        vertices[index as usize + 4 * i + 0].set_tex_coords(&quad_info.texture_coords[0]);
        vertices[index as usize + 4 * i + 0].set_color(&quad_info.colors[0]);

        vertices[index as usize + 4 * i + 1].set_pos(&GL_SPoint {
            x: quads[i].x + quads[i].width,
            y: quads[i].y,
        });
        vertices[index as usize + 4 * i + 1].set_tex_coords(&quad_info.texture_coords[1]);
        vertices[index as usize + 4 * i + 1].set_color(&quad_info.colors[1]);

        vertices[index as usize + 4 * i + 2].set_pos(&GL_SPoint {
            x: quads[i].x + quads[i].width,
            y: quads[i].y + quads[i].height,
        });
        vertices[index as usize + 4 * i + 2].set_tex_coords(&quad_info.texture_coords[2]);
        vertices[index as usize + 4 * i + 2].set_color(&quad_info.colors[2]);

        vertices[index as usize + 4 * i + 3].set_pos(&GL_SPoint {
            x: quads[i].x,
            y: quads[i].y + quads[i].height,
        });
        vertices[index as usize + 4 * i + 3].set_tex_coords(&quad_info.texture_coords[3]);
        vertices[index as usize + 4 * i + 3].set_color(&quad_info.colors[3]);

        if quad_info.rotation != 0.0 {
            let mut center = vec2::default();
            center.x = quads[i].x + quads[i].width / 2.0;
            center.y = quads[i].y + quads[i].height / 2.0;

            rotate(
                &center,
                quad_info.rotation,
                &mut vertices[index as usize + 4 * i..(index as usize + 4 * i) + 4],
            );
        }

        *vertices_count += 4;
    }
}

pub fn triangle_draw_tl_impl<T>(tri_info: &mut DrawScope<3>, triangles: &[Triangle])
where
    T: WriteVertexAttributes,
{
    let backend_handle = tri_info.backend_handle.backend_buffer_mut();
    let (vertices, vertices_count) = backend_handle.vertices_and_count_mut();
    for i in 0..triangles.len() {
        let index = *vertices_count;
        vertices[index as usize + 3 * i + 0].set_pos(&GL_SPoint {
            x: triangles[i].vertices[0].x,
            y: triangles[i].vertices[0].y,
        });
        vertices[index as usize + 3 * i + 0].set_tex_coords(&tri_info.texture_coords[0]);
        vertices[index as usize + 3 * i + 0].set_color(&tri_info.colors[0]);

        vertices[index as usize + 3 * i + 1].set_pos(&GL_SPoint {
            x: triangles[i].vertices[1].x,
            y: triangles[i].vertices[1].y,
        });
        vertices[index as usize + 3 * i + 1].set_tex_coords(&tri_info.texture_coords[1]);
        vertices[index as usize + 3 * i + 1].set_color(&tri_info.colors[1]);

        vertices[index as usize + 3 * i + 2].set_pos(&GL_SPoint {
            x: triangles[i].vertices[2].x,
            y: triangles[i].vertices[2].y,
        });
        vertices[index as usize + 3 * i + 2].set_tex_coords(&tri_info.texture_coords[2]);
        vertices[index as usize + 3 * i + 2].set_color(&tri_info.colors[2]);

        *vertices_count += 3;
    }
}

pub fn lines_draw_tl_impl<T>(quad_info: &mut DrawScope<2>, lines: &[Line])
where
    T: WriteVertexAttributes,
{
    let backend_handle = quad_info.backend_handle.backend_buffer_mut();
    let (vertices, vertices_count) = backend_handle.vertices_and_count_mut();
    for i in 0..lines.len() {
        let index = *vertices_count;
        vertices[index as usize + 2 * i + 0].set_pos(&GL_SPoint {
            x: lines[i].vertices[0].x,
            y: lines[i].vertices[0].y,
        });
        vertices[index as usize + 2 * i + 1].set_pos(&GL_SPoint {
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

pub struct DrawScope<'a, const VERTEX_COUNT: usize> {
    pub state: State,
    pub tile_index: u8,
    pub rotation: f32,
    pub vertices_offset: usize,
    pub backend_handle: &'a mut dyn GraphicsStreamHandler,
    pub colors: [SColor; VERTEX_COUNT],
    pub texture_coords: [STexCoord; VERTEX_COUNT],
}

impl<'a, const VERTEX_COUNT: usize> DrawScope<'a, VERTEX_COUNT> {
    pub fn new(backend_handle: &'a mut dyn GraphicsStreamHandler, vertices_offset: usize) -> Self {
        Self {
            state: State::new(),
            backend_handle: backend_handle,
            vertices_offset: vertices_offset,
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

    pub fn set_texture(&mut self, tex_index: ETextureIndex) {
        self.state.set_texture(tex_index);
    }

    pub fn blend_none(&mut self) {
        self.state.blend_none();
    }

    pub fn blend_normal(&mut self) {
        self.state.blend_normal();
    }

    pub fn blend_additive(&mut self) {
        self.state.blend_additive();
    }

    pub fn clip(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.state.clip(x, y, w, h);
    }

    pub fn wrap_clamp(&mut self) {
        self.state.wrap_clamp();
    }

    pub fn wrap_normal(&mut self) {
        self.state.wrap_normal();
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

    pub fn get_canvas_mapping(
        &self,
        top_left_x: &mut f32,
        top_left_y: &mut f32,
        bottom_right_x: &mut f32,
        bottom_right_y: &mut f32,
    ) {
        self.state
            .get_canvas_mapping(top_left_x, top_left_y, bottom_right_x, bottom_right_y);
    }

    pub fn set_rotation(&mut self, angle: f32) {
        self.rotation = angle;
    }

    /**
     * Tries to create a slice of vertices required for of the given number of primitives
     * Might however not return that size
     */
    pub fn get_raw_handle(&mut self, number_of_prims: usize) -> &mut [GL_SVertex] {
        if number_of_prims * VERTEX_COUNT
            > self
                .backend_handle
                .backend_buffer_mut()
                .vertices_mut()
                .len()
                - self.backend_handle.backend_buffer_mut().vertices_count()
        {
            self.backend_handle.flush_vertices(
                &self.state,
                self.vertices_offset,
                DrawModes::Triangles,
            );
            self.backend_handle.run_backend_buffer();
        }
        let offset = self.backend_handle.backend_buffer_mut().vertices_count();
        let number_of_vert = std::cmp::min(
            number_of_prims * VERTEX_COUNT,
            ((self
                .backend_handle
                .backend_buffer_mut()
                .vertices_mut()
                .len()
                - self.backend_handle.backend_buffer_mut().vertices_count())
                / VERTEX_COUNT)
                * VERTEX_COUNT,
        );
        *self
            .backend_handle
            .backend_buffer_mut()
            .vertices_count_mut() += number_of_vert;
        let backend_handle = self.backend_handle.backend_buffer_mut();
        let vertices_count = backend_handle.vertices_count();
        &mut backend_handle.vertices_mut()[offset..vertices_count]
    }
}

impl<'a, const VERTEX_COUNT: usize> Drop for DrawScope<'a, VERTEX_COUNT> {
    fn drop(&mut self) {}
}

pub trait DrawScopeImpl<'a, 'b, const VERTEX_COUNT: usize>
where
    'a: 'b,
{
    fn get_draw_scope(&'b mut self) -> &'b mut DrawScope<'a, VERTEX_COUNT>;

    fn set_colors_from_single(&'b mut self, r: f32, g: f32, b: f32, a: f32) {
        self.get_draw_scope().set_colors_from_single(r, g, b, a);
    }

    fn set_colors(&'b mut self, colors: &[vec4; VERTEX_COUNT]) {
        self.get_draw_scope().set_colors(colors);
    }

    fn set_state(&'b mut self, state: &State) {
        self.get_draw_scope().set_state(state);
    }

    fn set_texture(&'b mut self, tex_index: ETextureIndex) {
        self.get_draw_scope().set_texture(tex_index);
    }

    fn wrap_clamp(&'b mut self) {
        self.get_draw_scope().wrap_clamp();
    }

    fn blend_additive(&'b mut self) {
        self.get_draw_scope().blend_additive();
    }

    fn clip(&'b mut self, x: i32, y: i32, w: u32, h: u32) {
        self.get_draw_scope().clip(x, y, w, h);
    }

    fn map_canvas(
        &'b mut self,
        top_left_x: f32,
        top_left_y: f32,
        bottom_right_x: f32,
        bottom_right_y: f32,
    ) {
        self.get_draw_scope()
            .map_canvas(top_left_x, top_left_y, bottom_right_x, bottom_right_y);
    }

    fn get_canvas_mapping(
        &'b mut self,
        top_left_x: &mut f32,
        top_left_y: &mut f32,
        bottom_right_x: &mut f32,
        bottom_right_y: &mut f32,
    ) {
        self.get_draw_scope().get_canvas_mapping(
            top_left_x,
            top_left_y,
            bottom_right_x,
            bottom_right_y,
        );
    }

    fn get_raw_handle(&'b mut self, number_of_prims: usize) -> &'b mut [GL_SVertex] {
        self.get_draw_scope().get_raw_handle(number_of_prims)
    }
}

pub struct DrawQuads<'a> {
    pub draw_scope: DrawScope<'a, 4>,
}

impl<'a> DrawQuads<'a> {
    pub fn new(backend_handle: &'a mut dyn GraphicsStreamHandler, vertices_offset: usize) -> Self {
        Self {
            draw_scope: DrawScope::new(backend_handle, vertices_offset),
        }
    }

    pub fn from_draw_scope(draw_scope: DrawScope<'a, 4>) -> Self {
        Self {
            draw_scope: draw_scope,
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

    pub fn quads_draw_tl(&mut self, quads: &[CQuadItem]) {
        let _vert_dummy = Vec::<GL_SVertex>::new();
        if quads.len() * 4
            > self
                .draw_scope
                .backend_handle
                .backend_buffer_mut()
                .vertices_mut()
                .len()
                - self
                    .draw_scope
                    .backend_handle
                    .backend_buffer_mut()
                    .vertices_count()
        {
            self.draw_scope.backend_handle.flush_vertices(
                &self.draw_scope.state,
                self.draw_scope.vertices_offset,
                DrawModes::Quads,
            );
            self.draw_scope.backend_handle.run_backend_buffer();

            if quads.len() * 4
                > self
                    .draw_scope
                    .backend_handle
                    .backend_buffer_mut()
                    .vertices_mut()
                    .len()
                    - self
                        .draw_scope
                        .backend_handle
                        .backend_buffer_mut()
                        .vertices_count()
            {
                panic!("rendered too many vertices at once.");
            }
        }
        quads_draw_tl_impl::<SVertex>(&mut self.draw_scope, quads);
    }
}

impl<'a> Drop for DrawQuads<'a> {
    fn drop(&mut self) {
        self.draw_scope.backend_handle.flush_vertices(
            &self.draw_scope.state,
            self.draw_scope.vertices_offset,
            DrawModes::Quads,
        );
    }
}

impl<'a: 'b, 'b> DrawScopeImpl<'a, 'b, 4> for DrawQuads<'a> {
    fn get_draw_scope(&'b mut self) -> &'b mut DrawScope<'a, 4> {
        &mut self.draw_scope
    }
}

pub struct DrawTriangles<'a> {
    draw_scope: DrawScope<'a, 3>,
}

impl<'a> DrawTriangles<'a> {
    pub fn new(backend_handle: &'a mut dyn GraphicsStreamHandler, vertices_offset: usize) -> Self {
        Self {
            draw_scope: DrawScope::new(backend_handle, vertices_offset),
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
        let _vert_dummy = Vec::<GL_SVertex>::new();
        if triangles.len() * 3
            > self
                .draw_scope
                .backend_handle
                .backend_buffer_mut()
                .vertices_mut()
                .len()
                - self
                    .draw_scope
                    .backend_handle
                    .backend_buffer_mut()
                    .vertices_count()
        {
            self.draw_scope.backend_handle.flush_vertices(
                &self.draw_scope.state,
                self.draw_scope.vertices_offset,
                DrawModes::Triangles,
            );
            self.draw_scope.backend_handle.run_backend_buffer();
        }
        triangle_draw_tl_impl::<SVertex>(&mut self.draw_scope, triangles);
    }
}

impl<'a> Drop for DrawTriangles<'a> {
    fn drop(&mut self) {
        self.draw_scope.backend_handle.flush_vertices(
            &self.draw_scope.state,
            self.draw_scope.vertices_offset,
            DrawModes::Triangles,
        );
    }
}

impl<'a: 'b, 'b> DrawScopeImpl<'a, 'b, 3> for DrawTriangles<'a> {
    fn get_draw_scope(&'b mut self) -> &'b mut DrawScope<'a, 3> {
        &mut self.draw_scope
    }
}

pub struct DrawLines<'a> {
    draw_scope: DrawScope<'a, 2>,
}

impl<'a> DrawLines<'a> {
    pub fn new(backend_handle: &'a mut dyn GraphicsStreamHandler, vertices_offset: usize) -> Self {
        Self {
            draw_scope: DrawScope::new(backend_handle, vertices_offset),
        }
    }

    pub fn from_draw_scope(draw_scope: DrawScope<'a, 2>) -> Self {
        Self {
            draw_scope: draw_scope,
        }
    }

    pub fn lines_draw_tl(&mut self, lines: &[Line]) {
        let _vert_dummy = Vec::<GL_SVertex>::new();
        if lines.len() * 2
            > self
                .draw_scope
                .backend_handle
                .backend_buffer_mut()
                .vertices_mut()
                .len()
                - self
                    .draw_scope
                    .backend_handle
                    .backend_buffer_mut()
                    .vertices_count()
        {
            self.draw_scope.backend_handle.flush_vertices(
                &self.draw_scope.state,
                self.draw_scope.vertices_offset,
                DrawModes::Lines,
            );
            self.draw_scope.backend_handle.run_backend_buffer();

            if lines.len() * 2
                > self
                    .draw_scope
                    .backend_handle
                    .backend_buffer_mut()
                    .vertices_mut()
                    .len()
                    - self
                        .draw_scope
                        .backend_handle
                        .backend_buffer_mut()
                        .vertices_count()
            {
                panic!("rendered too many vertices at once.");
            }
        }
        lines_draw_tl_impl::<SVertex>(&mut self.draw_scope, lines);
    }
}

impl<'a> Drop for DrawLines<'a> {
    fn drop(&mut self) {
        self.draw_scope.backend_handle.flush_vertices(
            &self.draw_scope.state,
            self.draw_scope.vertices_offset,
            DrawModes::Lines,
        );
    }
}

impl<'a: 'b, 'b> DrawScopeImpl<'a, 'b, 2> for DrawLines<'a> {
    fn get_draw_scope(&'b mut self) -> &'b mut DrawScope<'a, 2> {
        &mut self.draw_scope
    }
}
