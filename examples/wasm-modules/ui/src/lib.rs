use api::graphics::graphics::Graphics;
use graphics_base::streaming::DrawScopeImpl;
use graphics_types::types::CQuadItem;

#[no_mangle]
fn mod_main(graphics: &mut Graphics) {
    let mut quad_scope = graphics.quads_begin();
    quad_scope.map_canvas(0.0, 0.0, 200.0, 200.0);
    quad_scope.set_colors_from_single(1.0, 0.0, 0.0, 0.25);
    quad_scope.quads_draw_tl(&[CQuadItem::new(50.0, 0.0, 100.0, 100.0)]);
}
