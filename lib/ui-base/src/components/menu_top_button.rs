use egui::{text::LayoutJob, Color32, FontId, Pos2, Response, Shape, Stroke, TextFormat};

pub struct MenuTopButtonProps {
    active: bool,
    text: String,
}

impl MenuTopButtonProps {
    pub fn new(text: &str, current_active: &Option<String>) -> Self {
        Self {
            active: Some(text).eq(&current_active.as_ref().map(|s| s.as_str())),
            text: text.to_string(),
        }
    }
}

#[must_use]
pub fn menu_top_button(ui: &mut egui::Ui, props: MenuTopButtonProps) -> Response {
    let res = ui.button(props.text);
    if props.active {
        ui.painter().add(Shape::line_segment(
            [
                Pos2::new(res.rect.left() + 8.0, res.rect.top() + 18.0),
                Pos2::new(
                    res.rect.left() + res.rect.width() - 8.0,
                    res.rect.top() + 18.0,
                ),
            ],
            Stroke::new(1.0, Color32::LIGHT_BLUE),
        ));
    }
    res
}

#[must_use]
pub fn menu_top_button_icon(ui: &mut egui::Ui, props: MenuTopButtonProps) -> Response {
    let mut job = LayoutJob::default();
    job.append(
        &props.text,
        0.0,
        TextFormat {
            font_id: FontId::new(12.0, egui::FontFamily::Name("icons".into())),
            color: ui.style().visuals.text_color(),
            ..Default::default()
        },
    );
    let res = ui.button(job);
    if props.active {
        ui.painter().add(Shape::line_segment(
            [
                Pos2::new(res.rect.left() + 8.0, 18.0),
                Pos2::new(res.rect.left() + res.rect.width() - 8.0, 18.0),
            ],
            Stroke::new(1.0, Color32::LIGHT_BLUE),
        ));
    }
    res
}

#[must_use]
pub fn text_icon(ui: &mut egui::Ui, text: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::new(12.0, egui::FontFamily::Name("icons".into())),
            color: ui.style().visuals.text_color(),
            ..Default::default()
        },
    );
    job
}
