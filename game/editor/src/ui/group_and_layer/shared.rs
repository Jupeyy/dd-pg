use crate::explain::TEXT_ANIM_PANEL_OPEN;

pub fn copy_tiles<T: Copy + Default>(
    old_width: usize,
    old_height: usize,
    new_width: usize,
    new_height: usize,
    old_tiles: &[T],
) -> Vec<T> {
    // change tiles
    let mut tiles: Vec<T> = Vec::new();
    tiles.resize(new_width * new_height, Default::default());
    old_tiles
        .chunks_exact(old_width)
        .enumerate()
        .take(old_height.min(new_height))
        .for_each(|(y, tile_chunk)| {
            let new_offset = y * new_width;
            let copy_width = old_width.min(new_width);
            tiles[new_offset..new_offset + copy_width].copy_from_slice(&tile_chunk[..copy_width]);
        });
    tiles
}

pub fn animations_panel_open_warning(ui: &mut egui::Ui) {
    let mut cache = egui_commonmark::CommonMarkCache::default();
    egui_commonmark::CommonMarkViewer::new("anim-panel-open-warning-tooltip").show(
        ui,
        &mut cache,
        TEXT_ANIM_PANEL_OPEN,
    );
}
