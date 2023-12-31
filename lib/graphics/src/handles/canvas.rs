use graphics_types::{
    commands::{
        AllCommands, CommandSwitchCanvasMode, CommandSwitchCanvasModeType, CommandUpdateViewport,
        Commands,
    },
    types::WindowProps,
};
use hiarc_macro::{hiarc_safer_rc_refcell, Hiarc};

use super::backend::GraphicsBackendHandle;

#[derive(Debug)]
pub struct GraphicsCanvas {
    window_props: WindowProps,
}

#[derive(Debug)]
pub struct GraphicsCanvasSetup {
    onscreen: GraphicsCanvas,
    offscreen: GraphicsCanvas,
}

#[derive(Debug)]
pub enum GraphicsCanvasMode {
    Onscreen,
    Offscreen,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphicsViewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Hiarc)]
pub struct GraphicsCanvasHandle {
    #[hiarc]
    backend_handle: GraphicsBackendHandle,

    canvases: GraphicsCanvasSetup,
    cur_canvas_mode: GraphicsCanvasMode,

    cur_dynamic_viewport: Option<GraphicsViewport>,
}

#[hiarc_safer_rc_refcell]
impl GraphicsCanvasHandle {
    pub fn new(backend_handle: GraphicsBackendHandle, window_props: WindowProps) -> Self {
        Self {
            backend_handle,
            canvases: GraphicsCanvasSetup {
                onscreen: GraphicsCanvas { window_props },
                offscreen: GraphicsCanvas { window_props },
            },
            cur_canvas_mode: GraphicsCanvasMode::Onscreen,

            cur_dynamic_viewport: None,
        }
    }

    pub fn resized(&mut self, window_props: WindowProps) {
        self.canvases.onscreen.window_props = window_props;
    }

    pub fn switch_canvas(&mut self, mode: CommandSwitchCanvasModeType) {
        match &mode {
            CommandSwitchCanvasModeType::Offscreen { .. } => {
                self.cur_canvas_mode = GraphicsCanvasMode::Offscreen;
            }
            CommandSwitchCanvasModeType::Onscreen => {
                self.cur_canvas_mode = GraphicsCanvasMode::Onscreen;
            }
        }
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::SwitchCanvas(
                CommandSwitchCanvasMode { mode },
            )));
    }

    /// update the viewport of the window where the origin is top left
    /// the dynamic viewport will affect calls to canvas_width-/height aswell
    /// as window_width-/height
    pub fn update_window_viewport(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let cmd = CommandUpdateViewport {
            x,
            y,
            width,
            height,
            by_resize: false,
        };
        self.backend_handle
            .add_cmd(AllCommands::Misc(Commands::UpdateViewport(cmd)));
        self.cur_dynamic_viewport = Some(GraphicsViewport {
            x,
            y,
            width,
            height,
        });
        let cur_canvas = &self.get_cur_canvas().window_props;
        if x == 0
            && y == 0
            && width == cur_canvas.window_width as u32
            && height == cur_canvas.window_height as u32
        {
            self.cur_dynamic_viewport = None;
        }
    }

    /// reset the viewport to the original window viewport
    pub fn reset_window_viewport(&mut self) {
        let window_props = self.window_props();
        self.update_window_viewport(0, 0, window_props.window_width, window_props.window_height)
    }

    fn get_cur_canvas(&self) -> &GraphicsCanvas {
        match self.cur_canvas_mode {
            GraphicsCanvasMode::Onscreen => &self.canvases.onscreen,
            GraphicsCanvasMode::Offscreen => &self.canvases.offscreen,
        }
    }

    /// get the current dynamic viewport, if any
    pub fn dynamic_viewport(&self) -> Option<GraphicsViewport> {
        self.cur_dynamic_viewport
    }

    /// the aspect of the window canvas, independent of the current viewport
    /// this function should generally __not__ be used over `canvas_aspect`,
    /// except you know what you are doing
    pub fn window_canvas_aspect(&self) -> f32 {
        let canvas = self.get_cur_canvas();
        (canvas.window_props.canvas_width / canvas.window_props.canvas_height) as f32
    }

    /// the width of the window canvas, independent of the current viewport
    /// this function should generally __not__ be used over `canvas_width`,
    /// except you know what you are doing
    pub fn window_canvas_width(&self) -> f32 {
        let canvas = self.get_cur_canvas();
        canvas.window_props.canvas_width as f32
    }

    /// the height of the window canvas, independent of the current viewport
    /// this function should generally __not__ be used over `canvas_height`,
    /// except you know what you are doing
    pub fn window_canvas_height(&self) -> f32 {
        let canvas = self.get_cur_canvas();
        canvas.window_props.canvas_height as f32
    }

    /// this is the aspect of the canvas you are currently able to draw on
    /// it respects the current mapped viewport
    /// generally you should use this function of `window_canvas_aspect` except
    /// you need to know the aspect of the _real_ canvas
    pub fn canvas_aspect(&self) -> f32 {
        self.cur_dynamic_viewport
            .as_ref()
            .map(|vp| (vp.width as f32 / vp.height as f32))
            .unwrap_or(self.window_canvas_aspect())
    }

    /// this is the width of the canvas you are currently able to draw on
    /// it respects the current mapped viewport
    /// generally you should use this function of `window_canvas_width` except
    /// you need to know the width of the _real_ canvas
    pub fn canvas_width(&self) -> f32 {
        self.cur_dynamic_viewport
            .as_ref()
            .map(|vp| vp.width as f32 / self.window_pixels_per_point())
            .unwrap_or(self.window_canvas_width())
    }

    /// this is the height of the canvas you are currently able to draw on
    /// it respects the current mapped viewport
    /// generally you should use this function of `window_canvas_height` except
    /// you need to know the height of the _real_ canvas
    pub fn canvas_height(&self) -> f32 {
        self.cur_dynamic_viewport
            .as_ref()
            .map(|vp| vp.height as f32 / self.window_pixels_per_point())
            .unwrap_or(self.window_canvas_height())
    }

    /// this function always respects the current viewport
    /// if you want to acess the real width use `window_props`
    pub fn window_width(&self) -> u32 {
        self.cur_dynamic_viewport
            .as_ref()
            .map(|vp| vp.width)
            .unwrap_or({
                let canvas = self.get_cur_canvas();
                canvas.window_props.window_width
            })
    }

    /// this function always respects the current viewport
    /// if you want to acess the real height use `window_props`
    pub fn window_height(&self) -> u32 {
        self.cur_dynamic_viewport
            .as_ref()
            .map(|vp| vp.height)
            .unwrap_or({
                let canvas = self.get_cur_canvas();
                canvas.window_props.window_height
            })
    }

    pub fn window_props(&self) -> WindowProps {
        let canvas = self.get_cur_canvas();
        canvas.window_props
    }

    pub fn window_pixels_per_point(&self) -> f32 {
        let canvas = self.get_cur_canvas();
        canvas.window_props.window_width as f32 / canvas.window_props.canvas_width as f32
    }
}
