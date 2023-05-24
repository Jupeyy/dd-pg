use graphics::graphics::Graphics;
use wasm_runtime::WasmManager;

pub struct UIManager {
    manager: WasmManager,
}

impl UIManager {
    pub fn new() -> Self {
        let wasm_bytes = include_bytes!("../../../target/wasm32-unknown-unknown/debug/ui.wasm");

        let mut manager = WasmManager::new(wasm_bytes).unwrap();

        Self { manager }
    }

    pub fn run(&mut self, graphics: &mut Graphics) {
        self.manager.run(graphics).unwrap();
    }
}
