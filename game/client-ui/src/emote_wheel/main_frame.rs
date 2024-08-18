use egui::{pos2, vec2, Color32, Id, Stroke};
use game_interface::types::{
    emoticons::{EmoticonType, EnumCount},
    render::character::{IntoEnumIterator, TeeEye},
};
use math::math::{vector::vec2, PI};
use ui_base::types::{UiRenderPipe, UiState};

use crate::utils::{render_emoticon_for_ui, render_tee_for_ui, rotate};

use super::user_data::UserData;

/// not required
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    let rect = ui.ctx().screen_rect();

    let width_scale = rect.width() / pipe.user_data.canvas_handle.canvas_width();

    let radius = |percentage: f32| {
        (percentage / 100.0 * pipe.user_data.canvas_handle.canvas_height()) * width_scale
    };

    let color = if main_frame_only {
        // no transparency for main frame
        Color32::BLACK
    } else {
        Color32::from_black_alpha(100)
    };

    let outer_radius = radius(35.0);

    ui.painter()
        .circle_filled(rect.center(), outer_radius, color);

    let inner_center = radius(15.0) / 2.0 + radius(5.0);
    let inner_stroke_size = radius(15.0);

    ui.painter().circle_stroke(
        rect.center(),
        inner_center,
        Stroke::new(inner_stroke_size, color),
    );

    if main_frame_only {
        return;
    }

    // render emoticons in a radius around the outer circle
    let mut pos = vec2::new(
        0.0,
        outer_radius - (outer_radius - (inner_center + inner_stroke_size / 2.0)) / 2.0,
    );
    let center = rect.center();
    let center = vec2::new(center.x, center.y);

    // rotate a bit so oop eyes are on the very right
    rotate(
        &vec2::default(),
        -2.0 * 5.0 / EmoticonType::COUNT as f32 * PI,
        std::slice::from_mut(&mut pos),
    );
    for emote in EmoticonType::iter() {
        rotate(
            &vec2::default(),
            2.0 / EmoticonType::COUNT as f32 * PI,
            std::slice::from_mut(&mut pos),
        );
        let center = center + pos;
        let size = radius(10.0);
        let selected = ui.input(|i| {
            i.pointer.hover_pos().is_some_and(|p| {
                // smaller hitbox, because there are overlappings.
                let size = size * 0.875;
                egui::Rect::from_center_size(pos2(center.x, center.y), vec2(size, size)).contains(p)
            })
        });
        let val = if selected {
            ui.ctx().animate_value_with_time(
                Id::new(format!("emote-wheel-anims-emoticons-{}", emote as usize)),
                1.5,
                0.15,
            )
        } else {
            ui.ctx().animate_value_with_time(
                Id::new(format!("emote-wheel-anims-emoticons-{}", emote as usize)),
                1.0,
                0.15,
            )
        };
        render_emoticon_for_ui(
            pipe.user_data.stream_handle,
            pipe.user_data.canvas_handle,
            pipe.user_data.emoticons_container,
            ui,
            ui_state,
            rect,
            None,
            pipe.user_data.emoticon,
            center,
            size * val,
            emote,
        );
    }

    // render tees in a radius around the inner circle
    let mut pos = vec2::new(0.0, inner_center);
    let center = rect.center();
    let center = vec2::new(center.x, center.y);

    // rotate a bit so normal eyes are on the very right
    rotate(
        &vec2::default(),
        -1.0 * 3.0 / TeeEye::COUNT as f32 * PI,
        std::slice::from_mut(&mut pos),
    );
    for eye in TeeEye::iter().rev() {
        rotate(
            &vec2::default(),
            2.0 / TeeEye::COUNT as f32 * PI,
            std::slice::from_mut(&mut pos),
        );
        let center = center + pos;
        let size = radius(10.0);
        let selected = ui.input(|i| {
            i.pointer.hover_pos().is_some_and(|p| {
                egui::Rect::from_center_size(pos2(center.x, center.y), vec2(size, size)).contains(p)
            })
        });
        let val = if selected {
            ui.ctx().animate_value_with_time(
                Id::new(format!("emote-wheel-anims-eyes-{}", eye as usize)),
                1.5,
                0.15,
            )
        } else {
            ui.ctx().animate_value_with_time(
                Id::new(format!("emote-wheel-anims-eyes-{}", eye as usize)),
                1.0,
                0.15,
            )
        };
        render_tee_for_ui(
            pipe.user_data.canvas_handle,
            pipe.user_data.skin_container,
            pipe.user_data.render_tee,
            ui,
            ui_state,
            rect,
            None,
            pipe.user_data.skin,
            pipe.user_data.skin_info.as_ref(),
            center,
            size * val,
            eye,
        );
    }
}
