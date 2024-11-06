use std::fmt::Debug;

pub trait CustomCallbackTrait: Debug + 'static {
    fn render(&self);
}
