use graphics_base::streaming::{DrawLines, DrawQuads, DrawScope, DrawTriangles};
use graphics_traits::GraphicsSizeQuery;
use graphics_types::types::QuadContainerIndex;

pub trait GraphicsRenderGeometry {
    fn lines_begin(&mut self) -> DrawLines;
    fn triangles_begin(&mut self) -> DrawTriangles;
    fn quads_begin(&mut self) -> DrawQuads;
    fn quads_tex_3d_begin(&mut self) -> DrawQuads;
    fn quad_scope_begin(&mut self) -> DrawScope<4>;
}

pub trait GraphicsRenderQuadContainer {
    fn render_quad_container_as_sprite(
        &mut self,
        container_index: &QuadContainerIndex,
        quad_offset: usize,
        x: f32,
        y: f32,
        scale_x: f32,
        scale_y: f32,
        quad_scope: DrawScope<4>,
    );
}

pub trait GraphicsRenderHandles {
    fn get_render_handles(
        &mut self,
    ) -> (
        &mut dyn GraphicsRenderGeometry,
        &mut dyn GraphicsRenderQuadContainer,
    );
}

pub trait GraphicsInterface: GraphicsSizeQuery + GraphicsRenderHandles {}
