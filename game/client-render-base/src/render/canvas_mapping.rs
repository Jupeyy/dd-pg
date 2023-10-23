use graphics_base_traits::traits::GraphicsSizeQuery;
use graphics_types::rendering::State;

use crate::map::render_tools::RenderTools;

pub fn map_canvas_for_ingame_items(
    graphics: &dyn GraphicsSizeQuery,
    state: &mut State,
    center_x: f32,
    center_y: f32,
    zoom: f32,
) {
    let points: [f32; 4] = RenderTools::map_canvas_to_world(
        0.0,
        0.0,
        0.0,
        0.0,
        100.0,
        center_x,
        center_y,
        graphics.canvas_aspect(),
        zoom,
    );
    state.map_canvas(points[0], points[1], points[2], points[3]);
}
