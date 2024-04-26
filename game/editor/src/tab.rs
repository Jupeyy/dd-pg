use client_render_base::map::map::RenderMap;

use crate::{client::EditorClient, map::EditorMap, server::EditorServer};

/// a tab, representing a map that is currently edited
pub struct EditorTab {
    pub map: EditorMap,
    pub map_render: RenderMap,
    pub server: Option<EditorServer>,
    pub client: EditorClient,
}
