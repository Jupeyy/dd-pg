use std::sync::Arc;

pub trait CustomCallbackTrait<P1, P2, P3>: 'static {
    fn render1(&self, _callback_custom_type1: &mut P1) {
        panic!("not implemented")
    }
    fn render2(&self, _callback_custom_type1: &mut P1, _callback_custom_type2: &mut P2) {
        panic!("not implemented")
    }
    fn render3(
        &self,
        _callback_custom_type1: &mut P1,
        _callback_custom_type2: &mut P2,
        _callback_custom_type3: &mut P3,
    ) {
        panic!("not implemented")
    }
}

pub struct CustomCallback<C1, C2, C3> {
    pub(crate) cb: Box<dyn CustomCallbackTrait<C1, C2, C3>>,
    pub custom_type_count: usize,
}

impl<C1: 'static, C2: 'static, C3: 'static> CustomCallback<C1, C2, C3> {
    pub fn new(
        cb: Box<dyn CustomCallbackTrait<C1, C2, C3>>,
        custom_type_count: usize,
        render_rect: egui::Rect,
    ) -> egui::PaintCallback {
        egui::PaintCallback {
            rect: render_rect,
            callback: Arc::new(Self {
                cb,
                custom_type_count,
            }),
        }
    }
}

// we don't use custom callback in threaded context
unsafe impl<C1, C2, C3> Sync for CustomCallback<C1, C2, C3> {}
unsafe impl<C1, C2, C3> Send for CustomCallback<C1, C2, C3> {}
