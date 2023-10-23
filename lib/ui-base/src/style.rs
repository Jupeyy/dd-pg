use egui::{Color32, Stroke, Style};

pub fn default_style() -> Style {
    let mut style = Style::default();
    style.visuals.dark_mode = true;
    style.visuals.window_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 128);
    style.visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0, Color32::from_rgb(200, 200, 200));
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(200, 200, 200));
    style
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
