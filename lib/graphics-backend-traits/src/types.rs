use std::{cell::RefCell, rc::Rc};

use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{command_buffer::AllCommands, rendering::SVertex};

#[derive(Debug, Clone)]
pub struct BackendCommands {
    cmds: Rc<RefCell<Vec<AllCommands>>>,
}

impl Default for BackendCommands {
    fn default() -> Self {
        Self {
            cmds: Rc::new(RefCell::new(Vec::with_capacity(200))),
        }
    }
}

impl BackendCommands {
    pub fn add_cmd(&self, cmd: AllCommands) {
        self.cmds.borrow_mut().push(cmd);
    }

    pub fn add_cmds(&self, cmds: &mut Vec<AllCommands>) {
        self.cmds.borrow_mut().append(cmds);
    }

    pub fn take(&self) -> Vec<AllCommands> {
        self.cmds.take()
    }

    pub fn replace(&self, swap: &mut Vec<AllCommands>) {
        std::mem::swap(&mut *self.cmds.borrow_mut(), swap);
    }
}

#[derive(Debug)]
pub struct BackendStreamData {
    pub vertices: &'static mut [SVertex],
    pub num_vertices: usize,
}

impl Default for BackendStreamData {
    fn default() -> Self {
        Self {
            vertices: &mut [],
            num_vertices: 0,
        }
    }
}

impl GraphicsStreamDataInterface for BackendStreamData {
    fn vertices(&self) -> &[SVertex] {
        &self.vertices
    }

    fn vertices_mut(&mut self) -> &mut [SVertex] {
        &mut self.vertices
    }

    fn vertices_count(&self) -> usize {
        self.num_vertices
    }

    fn vertices_count_mut(&mut self) -> &mut usize {
        &mut self.num_vertices
    }

    fn vertices_and_count(&self) -> (&[SVertex], &usize) {
        (&self.vertices, &self.num_vertices)
    }

    fn vertices_and_count_mut(&mut self) -> (&mut [SVertex], &mut usize) {
        (&mut self.vertices, &mut self.num_vertices)
    }

    fn set_vertices_unsafe(&mut self, vertices: &'static mut [SVertex]) {
        self.num_vertices = 0;
        self.vertices = vertices;
    }
}
