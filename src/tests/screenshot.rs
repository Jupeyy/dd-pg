use std::{cell::RefCell, path::Path, rc::Rc};

use graphics::graphics::graphics::{Graphics, ScreenshotCb};
use graphics_backend::backend::GraphicsBackend;
use graphics_backend_traits::traits::GraphicsBackendInterface;

pub fn save_screenshot(graphics: &Graphics, graphics_backend: &GraphicsBackend, name: &str) {
    #[derive(Debug)]
    struct Screenshot {
        file: Rc<RefCell<Option<anyhow::Result<Vec<u8>>>>>,
    }
    impl ScreenshotCb for Screenshot {
        fn on_screenshot(&self, png: anyhow::Result<Vec<u8>>) {
            *self.file.borrow_mut() = Some(png);
        }
    }
    let file: Rc<RefCell<Option<anyhow::Result<Vec<u8>>>>> = Default::default();
    let cb = Screenshot { file: file.clone() };
    graphics.do_screenshot(cb).unwrap();
    graphics.swap();
    graphics_backend.wait_idle().unwrap();
    graphics.check_pending_screenshot();
    let base_path: &Path = "artifacts/run".as_ref();
    std::fs::create_dir_all(base_path).unwrap();
    std::fs::write(
        base_path.join(name).with_extension(".png"),
        file.take().unwrap().unwrap(),
    )
    .unwrap();
}
