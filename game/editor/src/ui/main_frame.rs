use ui_base::types::{UIPipe, UIState};

use super::user_data::{UserData, UserDataWithTab};

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    main_frame_only: bool,
) {
    super::top_menu::menu::render(ui, pipe, ui_state, main_frame_only);

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
        };
        let mut pipe = UIPipe {
            cur_time: pipe.cur_time,
            user_data: &mut user_data,
        };
        super::left_panel::panel::render(ui, &mut pipe, ui_state, main_frame_only);
        super::top_toolbar::toolbar::render(ui, &mut pipe, ui_state, main_frame_only);
        super::bottom_panel::panel::render(ui, &mut pipe, ui_state, main_frame_only);
        super::animation_panel::panel::render(ui, &mut pipe, ui_state, main_frame_only);
        super::group_and_layer::group_props::render(ui, &mut pipe, ui_state, main_frame_only);
        super::group_and_layer::layer_props::render(ui, &mut pipe, ui_state, main_frame_only);
        super::group_and_layer::quad_props::render(ui, &mut pipe, ui_state, main_frame_only);
    }

    *pipe.user_data.unused_rect = Some(ui.available_rect_before_wrap());
    if *pipe.user_data.pointer_is_used {
        *pipe.user_data.unused_rect = None;
    }
}
