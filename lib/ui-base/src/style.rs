use egui::{Color32, Stroke, Style, Visuals};

pub fn default_style() -> Style {
    let mut visuals = Visuals::dark();
    let clr = visuals.window_fill.to_srgba_unmultiplied();
    visuals.window_fill = Color32::from_rgba_unmultiplied(clr[0], clr[1], clr[2], 180);
    let clr = visuals.extreme_bg_color.to_srgba_unmultiplied();
    visuals.extreme_bg_color = Color32::from_rgba_unmultiplied(clr[0], clr[1], clr[2], 180);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(200, 200, 200));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(200, 200, 200));
    visuals.clip_rect_margin = 0.0;
    Style {
        visuals,
        ..Default::default()
    }
}

pub fn topbar_buttons() -> Style {
    let mut style = default_style();
    style.visuals.widgets.inactive.rounding = 0.0.into();
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 100);

    style.visuals.widgets.hovered.rounding = 0.0.into();
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;

    style.visuals.widgets.active.rounding = 0.0.into();
    style.visuals.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 100);
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;

    style.visuals.button_frame = false;

    style
}
