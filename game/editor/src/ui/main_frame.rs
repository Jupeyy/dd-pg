use ui_base::types::UiRenderPipe;

use super::user_data::{UserData, UserDataWithTab};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
    super::top_menu::menu::render(ui, pipe, main_frame_only);

    // groups & layers attr
    if let Some(tab) = pipe.user_data.editor_tab.as_deref_mut() {
        let mut user_data = UserDataWithTab {
            ui_events: pipe.user_data.ui_events,
            config: pipe.user_data.config,
            canvas_handle: pipe.user_data.canvas_handle,
            stream_handle: pipe.user_data.stream_handle,
            editor_tab: tab,
            tools: pipe.user_data.tools,
            pointer_is_used: pipe.user_data.pointer_is_used,
            io: pipe.user_data.io,
            tp: pipe.user_data.tp,
            graphics_mt: pipe.user_data.graphics_mt,
            buffer_object_handle: pipe.user_data.buffer_object_handle,
            backend_handle: pipe.user_data.backend_handle,
        };
        let mut pipe = UiRenderPipe {
            cur_time: pipe.cur_time,
            user_data: &mut user_data,
        };
        super::left_panel::panel::render(ui, &mut pipe, main_frame_only);
        super::top_toolbar::toolbar::render(ui, &mut pipe, main_frame_only);
        super::bottom_panel::panel::render(ui, &mut pipe, main_frame_only);
        super::animation_panel::panel::render(ui, &mut pipe, main_frame_only);
        super::group_and_layer::group_props::render(ui, &mut pipe, main_frame_only);
        super::group_and_layer::layer_props::render(ui, &mut pipe, main_frame_only);
        super::group_and_layer::quad_props::render(ui, &mut pipe, main_frame_only);
        super::group_and_layer::sound_props::render(ui, &mut pipe, main_frame_only);
    }

    *pipe.user_data.unused_rect = Some(ui.available_rect_before_wrap());
    if *pipe.user_data.pointer_is_used {
        *pipe.user_data.unused_rect = None;
    }

    *pipe.user_data.input_state = Some(ui.ctx().input(|inp| inp.clone()));
    *pipe.user_data.canvas_size = Some(ui.ctx().input(|inp| inp.screen_rect()));
}
