use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc, RwLock},
    time::Duration,
};

use egui::{Color32, FontData, FontDefinitions, FontFamily, TextureId, ViewportId};
use graphics::handles::texture::texture::TextureContainer;
use hiarc::Hiarc;

use crate::{custom_callback::CustomCallbackTrait, font_data::UiFontData, style::default_style};

use super::types::{UiRenderPipe, UiState};

pub fn gui_main_panel(main_panel_color: &Color32) -> egui::CentralPanel {
    let standard_frame = egui::containers::Frame {
        inner_margin: egui::Margin {
            left: 0.,
            right: 0.,
            top: 0.,
            bottom: 0.,
        },
        outer_margin: egui::Margin {
            left: 0.,
            right: 0.,
            top: 0.,
            bottom: 0.,
        },
        rounding: egui::Rounding {
            nw: 0.0,
            ne: 0.0,
            sw: 0.0,
            se: 0.0,
        },
        shadow: egui::epaint::Shadow::NONE,
        fill: *main_panel_color,
        stroke: egui::Stroke::NONE,
    };
    egui::CentralPanel::default().frame(standard_frame)
}

#[derive(Debug, Hiarc, Clone, Default)]
pub struct UiContext {
    pub egui_ctx: egui::Context,
    pub textures: Rc<RefCell<HashMap<TextureId, TextureContainer>>>,
}

#[derive(Default, PartialEq)]
pub struct UiCachedProps {
    window_width: u32,
    window_height: u32,
    window_pixels_per_point: f32,
}

#[derive(Clone)]
pub struct UiCachedOutput {
    pub rect: egui::Rect,
    pub output: egui::FullOutput,
    pub zoom_level: f32,
    pub custom_paints: HashMap<u64, Rc<dyn CustomCallbackTrait>>,
}

pub type RepaintListeners = Arc<RwLock<HashMap<ViewportId, Arc<AtomicBool>>>>;

/// Helps to create UIs in efficient matter
#[derive(Debug)]
pub struct UiCreator {
    pub font_definitions: Option<FontDefinitions>,
    pub context: UiContext,
    pub id_gen: Cell<u64>,
    pub repaint_listeners: RepaintListeners,
}

impl Default for UiCreator {
    fn default() -> Self {
        let repaint_listeners =
            Arc::new(RwLock::new(<HashMap<ViewportId, Arc<AtomicBool>>>::new()));
        let context = egui::Context::default();
        context.options_mut(|option| option.zoom_with_keyboard = false);
        let repaint_listeners_cb = repaint_listeners.clone();
        context.set_request_repaint_callback(move |cb| {
            if let Some(should_repaint) = repaint_listeners_cb.read().unwrap().get(&cb.viewport_id)
            {
                should_repaint.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
        context.options_mut(|option| option.zoom_with_keyboard = false);
        let context = UiContext {
            egui_ctx: context,
            ..Default::default()
        };

        Self {
            font_definitions: Default::default(),
            context,
            id_gen: Default::default(),
            repaint_listeners,
        }
    }
}

impl UiCreator {
    pub fn load_font(&mut self, shared_fonts: &Arc<UiFontData>) {
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "default_latin".to_owned(),
            FontData::from_owned(shared_fonts.latin.clone()),
        );
        fonts.font_data.insert(
            "default_asia".to_owned(),
            FontData::from_owned(shared_fonts.asia.clone()),
        );
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "default_latin".to_owned());

        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(1, "default_asia".to_owned());

        fonts.font_data.insert(
            "icons".to_owned(),
            FontData::from_owned(shared_fonts.icon.clone()),
        );
        fonts
            .families
            .insert(FontFamily::Name("icons".into()), vec!["icons".into()]);

        self.context.egui_ctx.set_fonts(fonts.clone());

        self.font_definitions = Some(fonts);
    }
}

/// UI is not a client component, it should be cleanly separated from any game logic (but can read it)
pub struct UiContainer {
    pub context: UiContext,
    pub stencil_context: UiContext,
    pub viewport_id: ViewportId,
    pub stencil_viewport_id: ViewportId,

    pub ui_state: UiState,

    pub main_panel_color: Color32,

    should_repaint: Arc<AtomicBool>,
    cached_props: UiCachedProps,
    cached_output: Option<UiCachedOutput>,
    pub last_clipped_primitives: Vec<egui::ClippedPrimitive>,
    /// should tesselate clipped primitives this frame
    pub should_tesselate: bool,
}

impl UiContainer {
    /// `zoom_level` is optional. and means that it overrides the default value
    /// which is calculated by the window
    pub fn new(zoom_level: Option<f32>, creator: &UiCreator) -> Self {
        let should_repaint: Arc<AtomicBool> = Default::default();

        let context = creator.context.clone();
        let stencil_context = creator.context.clone();

        let viewport_id = ViewportId::from_hash_of(creator.id_gen.get() + 1);
        creator.id_gen.set(creator.id_gen.get() + 1);
        let stencil_viewport_id = ViewportId::from_hash_of(creator.id_gen.get() + 1);
        creator.id_gen.set(creator.id_gen.get() + 1);

        creator
            .repaint_listeners
            .write()
            .unwrap()
            .insert(viewport_id, should_repaint.clone());

        let vis = egui::style::Visuals::dark();
        context.egui_ctx.set_visuals(vis.clone());
        stencil_context.egui_ctx.set_visuals(vis);

        Self {
            context,
            stencil_context,
            viewport_id,
            stencil_viewport_id,

            ui_state: UiState::new(zoom_level),

            main_panel_color: Color32::TRANSPARENT,

            should_repaint,
            cached_props: Default::default(),
            cached_output: Default::default(),
            last_clipped_primitives: Default::default(),
            should_tesselate: false,
        }
    }

    pub fn split_mut(
        &mut self,
        as_stencil: bool,
    ) -> (&mut UiState, &mut egui::Context, &mut Color32) {
        (
            &mut self.ui_state,
            if as_stencil {
                &mut self.stencil_context.egui_ctx
            } else {
                &mut self.context.egui_ctx
            },
            &mut self.main_panel_color,
        )
    }

    pub fn set_main_panel_color(&mut self, main_panel_color: &Color32) {
        *self.split_mut(false).2 = *main_panel_color;
    }

    /// returns the canvas rect, full output and current zoom level
    pub fn render<U>(
        &mut self,
        window_width: u32,
        window_height: u32,
        window_pixels_per_point: f32,
        render_func: impl FnOnce(&mut egui::Ui, &mut UiRenderPipe<U>, &mut UiState),
        pipe: &mut UiRenderPipe<U>,
        mut input: egui::RawInput,
        as_stencil: bool,
    ) -> (egui::Rect, egui::FullOutput, f32) {
        // tell our rendering engine to tesselate again
        self.should_tesselate = true;

        let viewport_id = if as_stencil {
            self.stencil_viewport_id
        } else {
            self.viewport_id
        };
        let (ui_state, egui_ctx, main_panel_color) = self.split_mut(as_stencil);
        let mut zoom_level = ui_state.zoom_level.unwrap_or(window_pixels_per_point);

        let zoom_diff = zoom_level / window_pixels_per_point;

        // first go through all events
        let mut hint_has_text_input = false;
        // scale the input events down
        input.events.retain_mut(|ev| match ev {
            egui::Event::PointerMoved(ev) => {
                *ev = egui::pos2(ev.x, ev.y) / zoom_diff;
                true
            }
            egui::Event::PointerButton {
                pos,
                button: _,
                pressed: _,
                modifiers: _,
            } => {
                *pos = egui::pos2(pos.x, pos.y) / zoom_diff;
                true
            }
            egui::Event::Text(_) => {
                hint_has_text_input = true;
                true
            }
            egui::Event::Zoom(extra_zoom_level) => {
                let incr_val = if *extra_zoom_level > 1.0 {
                    if zoom_level < 1.5 {
                        0.25
                    } else {
                        0.5
                    }
                } else if *extra_zoom_level < 1.0 {
                    if zoom_level > 1.5 {
                        -0.5
                    } else {
                        -0.25
                    }
                } else {
                    0.0
                };
                zoom_level = (zoom_level + incr_val)
                    .clamp(window_pixels_per_point - 0.5, window_pixels_per_point + 1.0);
                false
            }
            _ => true,
        });
        ui_state.hint_had_input = hint_has_text_input;

        let screen_rect = egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 0.0 },
            max: egui::Pos2 {
                x: window_width as f32 / zoom_level,
                y: window_height as f32 / zoom_level,
            },
        };
        input.screen_rect = if screen_rect.width() > 0.0 && screen_rect.height() > 0.0 {
            Some(screen_rect)
        } else {
            None
        };
        let cur_time_secs =
            pipe.cur_time.as_nanos() as f64 / (Duration::from_secs(1).as_nanos() as f64);
        input.time = Some(cur_time_secs);

        input.viewport_id = viewport_id;
        input.viewports.insert(
            input.viewport_id,
            egui::ViewportInfo {
                parent: Default::default(),
                title: Default::default(),
                events: Default::default(),
                native_pixels_per_point: Some(zoom_level),
                monitor_size: Default::default(),
                inner_rect: Default::default(),
                outer_rect: Default::default(),
                minimized: Default::default(),
                maximized: Default::default(),
                fullscreen: Default::default(),
                focused: Default::default(),
            },
        );
        if zoom_level == window_pixels_per_point {
            ui_state.zoom_level = None;
        } else {
            ui_state.zoom_level = Some(zoom_level);
        }
        (
            screen_rect,
            egui_ctx.run(input, |egui_ctx| {
                egui_ctx.set_style(default_style());
                gui_main_panel(main_panel_color)
                    .show(egui_ctx, |ui| render_func(ui, pipe, ui_state));
            }),
            zoom_level,
        )
    }

    /// Like [`Self::render`], but it remembers if any input
    /// changed (window props, raw input etc.).
    /// If nothing changed the `render_func` is never called.
    /// This is useful if you plan to not rerender every frame.
    /// Note that this only works if the UI does not rely on immediate changes
    /// by variables that are passed by a user (basically any state
    /// this function can not know about).
    /// Additionally it only checks for events in the `input` variable,
    /// all other props are ignored.
    /// returns the canvas rect, full output and current zoom level
    pub fn render_cached<U>(
        &mut self,
        window_width: u32,
        window_height: u32,
        window_pixels_per_point: f32,
        render_func: impl FnOnce(&mut egui::Ui, &mut UiRenderPipe<U>, &mut UiState),
        pipe: &mut UiRenderPipe<U>,
        input: egui::RawInput,
        as_stencil: bool,
        force_rerender: bool,
    ) -> (egui::Rect, egui::FullOutput, f32) {
        let new_cached = UiCachedProps {
            window_width,
            window_height,
            window_pixels_per_point,
        };
        if self.cached_props != new_cached
            || self.cached_output.is_none()
            || !input.events.is_empty()
            || self
                .should_repaint
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            || force_rerender
        {
            let (rect, output, zoom_level) = self.render(
                window_width,
                window_height,
                window_pixels_per_point,
                render_func,
                pipe,
                input.clone(),
                as_stencil,
            );
            self.cached_output = Some(UiCachedOutput {
                rect,
                output,
                zoom_level,
                custom_paints: self.ui_state.custom_paints.clone(),
            });
        }
        self.cached_props = new_cached;
        let output = self.cached_output.clone().unwrap();
        if let Some(cached_output) = self.cached_output.as_mut() {
            // never do a texture delta twice
            cached_output.output.textures_delta.clear();
            // also set the custom paints to the cached ones
            self.ui_state
                .custom_paints
                .clone_from(&cached_output.custom_paints);
        }
        (output.rect, output.output, output.zoom_level)
    }
}
