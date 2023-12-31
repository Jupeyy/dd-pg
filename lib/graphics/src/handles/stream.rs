use std::{
    cell::{RefCell, RefMut},
    marker::PhantomData,
    rc::Rc,
};

use graphics_base_traits::traits::GraphicsStreamDataInterface;
use graphics_types::{
    commands::{
        AllCommands, CommandRender, CommandsRender, CommandsRenderStream, PrimType, RenderCommand,
        RenderSpriteInfo, GRAPHICS_DEFAULT_UNIFORM_SIZE, GRAPHICS_MAX_UNIFORM_RENDER_COUNT,
    },
    rendering::{RenderMode, State},
    types::DrawModes,
};
use hiarc_macro::Hiarc;

use crate::streaming::{DrawLines, DrawQuads, DrawTriangles};

use super::backend::GraphicsBackendHandle;

#[derive(Debug, Hiarc)]
pub struct GraphicsStreamHandle {
    stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>>,

    #[hiarc]
    backend_handle: GraphicsBackendHandle,
}

impl Clone for GraphicsStreamHandle {
    fn clone(&self) -> Self {
        Self {
            stream_data: self.stream_data.clone(),
            backend_handle: self.backend_handle.clone(),
        }
    }
}

fn flush_vertices_impl<T>(
    stream_data: &mut dyn GraphicsStreamDataInterface,
    state: &State,
    draw_mode: DrawModes,
    vertices_offset: usize,
    cmd: &mut T,
) -> bool
where
    T: RenderCommand,
{
    let prim_type: PrimType;
    let prim_count: usize;
    let num_verts: usize;
    num_verts = stream_data.vertices_count() - vertices_offset;

    if num_verts == 0 {
        return false;
    }

    match draw_mode {
        DrawModes::Quads => {
            prim_type = PrimType::Quads;
            prim_count = num_verts / 4;
        }
        DrawModes::Lines => {
            prim_type = PrimType::Lines;
            prim_count = num_verts / 2;
        }
        DrawModes::Triangles => {
            prim_type = PrimType::Triangles;
            prim_count = num_verts / 3;
        }
    }

    cmd.set_state(*state);

    cmd.set_prim_type(prim_type);
    cmd.set_prim_count(prim_count);

    //TODO: m_pCommandBuffer->AddRenderCalls(1);
    return true;
}

pub struct GraphicsStreamedSpritesUniformWrapper<'a> {
    handle: RefMut<'a, dyn GraphicsStreamDataInterface>,
    instance: usize,
}

impl<'a> GraphicsStreamedSpritesUniformWrapper<'a> {
    pub fn get(&mut self) -> (&mut [RenderSpriteInfo], &mut usize, usize) {
        let w = self.handle.get_sprites_uniform_instance(self.instance);
        (w.sprites, w.used_count, self.instance)
    }
}

pub struct GraphicsStreamedUniformWrapper<'a, T> {
    handle: RefMut<'a, dyn GraphicsStreamDataInterface>,
    instance: usize,
    phantom: PhantomData<T>,
}

impl<'a, T> GraphicsStreamedUniformWrapper<'a, T> {
    pub fn get(&mut self) -> (&mut [T], &mut usize, usize) {
        assert!(
            std::mem::size_of::<T>()
                < GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE
        );
        let w = self
            .handle
            .get_arbitrary_uniform_instance(self.instance, std::mem::size_of::<T>());
        (
            unsafe {
                std::slice::from_raw_parts_mut::<T>(
                    w.raw.as_ptr() as *mut _,
                    (GRAPHICS_MAX_UNIFORM_RENDER_COUNT * GRAPHICS_DEFAULT_UNIFORM_SIZE)
                        / std::mem::size_of::<T>(),
                )
            },
            w.used_count,
            self.instance,
        )
    }
}

impl GraphicsStreamHandle {
    pub fn new(
        stream_data: Rc<RefCell<dyn GraphicsStreamDataInterface>>,
        backend_handle: GraphicsBackendHandle,
    ) -> Self {
        Self {
            stream_data,
            backend_handle,
        }
    }

    pub fn lines_begin<'a>(&'a mut self) -> DrawLines {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawLines::new(self, vertices_offset)
    }

    pub fn triangles_begin<'a>(&'a mut self) -> DrawTriangles {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawTriangles::new(self, vertices_offset)
    }

    pub fn quads_begin<'a>(&'a mut self) -> DrawQuads {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawQuads::new(self, vertices_offset)
    }

    pub fn quads_tex_3d_begin<'a>(&'a mut self) -> DrawQuads {
        let vertices_offset = self.stream_data.borrow().vertices_count();
        DrawQuads::new(self, vertices_offset)
    }

    pub fn flush_vertices(
        &mut self,
        state: &State,
        render_mode: &RenderMode,
        vertices_offset: usize,
        draw_mode: DrawModes,
    ) {
        let mut cmd = CommandRender::new(PrimType::Lines);
        if flush_vertices_impl(
            &mut *self.stream_data.borrow_mut(),
            state,
            draw_mode,
            vertices_offset,
            &mut cmd,
        ) {
            cmd.vertices_offset = vertices_offset;

            let render_cmd = match render_mode {
                RenderMode::Standard => CommandsRenderStream::Render(cmd),
                RenderMode::Blur {
                    blur_radius,
                    scale,
                    blur_color,
                } => CommandsRenderStream::RenderBlurred {
                    cmd,
                    blur_radius: *blur_radius,
                    scale: *scale,
                    blur_color: *blur_color,
                },
            };

            self.backend_handle
                .add_cmd(AllCommands::Render(CommandsRender::Stream(render_cmd)));
        }
    }

    pub fn flush_commands_and_reset_vertices(&mut self, vertices_offset: &mut usize) {
        self.backend_handle.run_backend_buffer(&self.stream_data);
        *vertices_offset = 0;
    }

    pub fn stream_data(&self) -> &Rc<RefCell<dyn GraphicsStreamDataInterface>> {
        &self.stream_data
    }

    pub fn get_sprites_uniform_instance(&self) -> GraphicsStreamedSpritesUniformWrapper {
        let mut handle = self.stream_data.borrow_mut();
        let mut instance = handle.allocate_uniform_instance();
        if instance.is_err() {
            drop(handle);
            self.backend_handle.run_backend_buffer(&self.stream_data);
            handle = self.stream_data.borrow_mut();
            instance = handle.allocate_uniform_instance();
        }
        let instance = instance.unwrap();
        GraphicsStreamedSpritesUniformWrapper { handle, instance }
    }

    pub fn get_uniform_instance<T>(&self) -> GraphicsStreamedUniformWrapper<T> {
        let mut handle = self.stream_data.borrow_mut();
        let mut instance = handle.allocate_uniform_instance();
        if instance.is_err() {
            drop(handle);
            self.backend_handle.run_backend_buffer(&self.stream_data);
            handle = self.stream_data.borrow_mut();
            instance = handle.allocate_uniform_instance();
        }
        let instance = instance.unwrap();
        GraphicsStreamedUniformWrapper::<T> {
            handle,
            instance,
            phantom: Default::default(),
        }
    }
}
