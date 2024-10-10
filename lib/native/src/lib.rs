pub mod input;
pub mod native;

#[cfg(test)]
mod test {
    use winit::keyboard::{KeyCode, PhysicalKey};

    use crate::input::binds::BindKey;

    #[test]
    fn bind_json_abuse() {
        dbg!(serde_json::to_string(&KeyCode::KeyA).unwrap());
        dbg!(serde_json::to_string(&BindKey::Key(PhysicalKey::Code(KeyCode::KeyA))).unwrap());
    }
}
