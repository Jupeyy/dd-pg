use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;

pub trait CustomCallbackTrait<B: GraphicsBackendInterface, P1, P2, P3>:
    Sync + Send + 'static
{
    fn render1(&self, graphics: &mut GraphicsBase<B>, callback_custom_type1: &mut P1) {
        panic!("not implemented")
    }
    fn render2(
        &self,
        graphics: &mut GraphicsBase<B>,
        callback_custom_type1: &mut P1,
        callback_custom_type2: &mut P2,
    ) {
        panic!("not implemented")
    }
    fn render3(
        &self,
        graphics: &mut GraphicsBase<B>,
        callback_custom_type1: &mut P1,
        callback_custom_type2: &mut P2,
        callback_custom_type3: &mut P3,
    ) {
        panic!("not implemented")
    }
}

pub struct CustomCallback<B: GraphicsBackendInterface, C1, C2, C3> {
    pub(crate) cb: Box<dyn CustomCallbackTrait<B, C1, C2, C3>>,
    pub custom_type_count: usize,
}

impl<B: GraphicsBackendInterface, C1, C2, C3> CustomCallback<B, C1, C2, C3> {
    pub fn new(cb: Box<dyn CustomCallbackTrait<B, C1, C2, C3>>, custom_type_count: usize) -> Self {
        Self {
            cb,
            custom_type_count,
        }
    }
}
