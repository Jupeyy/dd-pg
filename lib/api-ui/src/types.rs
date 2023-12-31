use ui_base::ui::UI as RealUI;

pub struct UIWinitWrapper {}

pub struct UI {
    pub ui: RealUI<UIWinitWrapper>,
}

impl UI {
    pub fn new(zoom_level: Option<f32>) -> Self {
        Self {
            ui: RealUI::new(UIWinitWrapper {}, zoom_level),
        }
    }
}
