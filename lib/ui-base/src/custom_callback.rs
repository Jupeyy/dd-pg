use graphics::graphics::Graphics;

pub trait CustomCallbackTrait<P1, P2, P3>: Sync + Send + 'static {
    fn render1(&self, _graphics: &mut Graphics, _callback_custom_type1: &mut P1) {
        panic!("not implemented")
    }
    fn render2(
        &self,
        _graphics: &mut Graphics,
        _callback_custom_type1: &mut P1,
        _callback_custom_type2: &mut P2,
    ) {
        panic!("not implemented")
    }
    fn render3(
        &self,
        _graphics: &mut Graphics,
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

impl<C1, C2, C3> CustomCallback<C1, C2, C3> {
    pub fn new(cb: Box<dyn CustomCallbackTrait<C1, C2, C3>>, custom_type_count: usize) -> Self {
        Self {
            cb,
            custom_type_count,
        }
    }
}
