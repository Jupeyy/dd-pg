use ui_base::ui::UI as RealUI;

#[derive(Default)]
pub struct UIStateAPI {}

pub struct UI {
    pub ui: RealUI<UIStateAPI>,
}

impl UI {
    /*fn new(zoom_level: f32) -> Self {
        Self {
            ui: RealUI::new(
                UIStateAPI {
                    events: Default::default(),
                },
                zoom_level,
            ),
        }
    }*/
}
