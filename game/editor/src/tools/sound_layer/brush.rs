use client_render_base::map::{map::RenderMap, render_tools::RenderTools};
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::backend::GraphicsBackendHandle,
        buffer_object::buffer_object::GraphicsBufferObjectHandle,
        canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
        texture::texture::TextureContainer,
    },
};
use graphics_types::rendering::State;
use hiarc::Hiarc;
use map::map::groups::layers::design::Sound;
use math::math::vector::{ffixed, ubvec4, vec2};

use crate::{
    actions::actions::{
        ActChangeSoundAttr, ActSoundLayerAddRemSounds, ActSoundLayerAddSounds, EditorAction,
    },
    client::EditorClient,
    map::{EditorLayer, EditorLayerUnionRef, EditorMap, EditorMapInterface},
    tools::{shared::in_radius, utils::render_rect},
    utils::{ui_pos_to_world_pos, UiCanvasSize},
};

use super::shared::{render_sound_points, SoundPointerDownPoint, SOUND_POINT_RADIUS};

#[derive(Debug, Hiarc)]
pub struct SoundBrushSounds {
    pub sounds: Vec<Sound>,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Hiarc)]
pub struct SoundSelection {
    pub is_background: bool,
    pub group: usize,
    pub layer: usize,
    pub sound_index: usize,
    pub sound: Sound,
    pub point: SoundPointerDownPoint,
    pub cursor_in_world_pos: Option<vec2>,
}

#[derive(Debug, Hiarc)]
pub enum SoundPointerDownState {
    None,
    /// sound corner/center point
    Point(SoundPointerDownPoint),
    /// selection of sounds
    Selection(vec2),
}

impl SoundPointerDownState {
    pub fn is_selection(&self) -> bool {
        matches!(self, Self::Selection(_))
    }
}

/// sound brushes are relative to where the mouse selected them
#[derive(Debug, Hiarc)]
pub struct SoundBrush {
    pub brush: Option<SoundBrushSounds>,

    /// this is the last sound selected (clicked on the corner selectors), this can be used
    /// for the animation to know the current sound
    pub last_selection: Option<SoundSelection>,
    pub last_active: Option<SoundSelection>,

    pub pointer_down_state: SoundPointerDownState,

    pub parallax_aware_brush: bool,
}

impl Default for SoundBrush {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundBrush {
    pub fn new() -> Self {
        Self {
            brush: Default::default(),
            last_selection: None,
            last_active: None,
            pointer_down_state: SoundPointerDownState::None,

            parallax_aware_brush: false,
        }
    }

    fn handle_brush_select(
        &mut self,
        ui_canvas: &UiCanvasSize,
        graphics_mt: &GraphicsMultiThreaded,
        buffer_object_handle: &GraphicsBufferObjectHandle,
        backend_handle: &GraphicsBackendHandle,
        canvas_handle: &GraphicsCanvasHandle,
        map: &EditorMap,
        fake_texture: &TextureContainer,
        latest_pointer: &egui::PointerState,
        current_pointer_pos: &egui::Pos2,
        latest_modifiers: &egui::Modifiers,
        client: &mut EditorClient,
    ) {
        let layer = map.active_layer();
        let (offset, parallax) = if let Some(layer) = &layer {
            layer.get_offset_and_parallax()
        } else {
            Default::default()
        };
        let Some(EditorLayerUnionRef::Design {
            layer: EditorLayer::Sound(layer),
            group_index,
            is_background,
            layer_index,
            ..
        }) = layer
        else {
            return;
        };

        let pointer_cur = vec2::new(current_pointer_pos.x, current_pointer_pos.y);

        let vec2 {
            x: mut x1,
            y: mut y1,
        } = ui_pos_to_world_pos(
            canvas_handle,
            ui_canvas,
            map.groups.user.zoom,
            vec2::new(pointer_cur.x, pointer_cur.y),
            map.groups.user.pos.x,
            map.groups.user.pos.y,
            offset.x,
            offset.y,
            parallax.x,
            parallax.y,
        );

        // if pointer was already down
        if let SoundPointerDownState::Selection(pointer_down) = &self.pointer_down_state {
            // find current layer
            let vec2 {
                x: mut x0,
                y: mut y0,
            } = pointer_down;

            if x0 > x1 {
                std::mem::swap(&mut x0, &mut x1);
            }
            if y0 > y1 {
                std::mem::swap(&mut y0, &mut y1);
            }

            // check if any sounds are in the selection
            let mut sounds: Vec<Sound> = Default::default();

            for sound in &layer.layer.sounds {
                if super::super::quad_layer::shared::in_box(&sound.pos, x0, y0, x1, y1) {
                    sounds.push(sound.clone());
                }
            }

            // if there is an selection, apply that
            if !sounds.is_empty() {
                let pointer_down = vec2::new(x0, y0);

                let x = -pointer_down.x;
                let y = -pointer_down.y;

                for sound in &mut sounds {
                    sound.pos.x += ffixed::from_num(x);
                    sound.pos.y += ffixed::from_num(y);
                }

                self.brush = Some(SoundBrushSounds {
                    sounds,
                    w: x1 - x0,
                    h: y1 - y0,
                });
            }

            if !latest_pointer.primary_down() {
                self.pointer_down_state = SoundPointerDownState::None;
            }
        } else {
            // check if the pointer clicked on one of the sound corner/center points
            let mut clicked_sound_point = false;
            if latest_pointer.primary_pressed() || latest_pointer.secondary_pressed() {
                for (s, sound) in layer.layer.sounds.iter().enumerate() {
                    let pointer_cur = vec2::new(current_pointer_pos.x, current_pointer_pos.y);

                    let pointer_cur = ui_pos_to_world_pos(
                        canvas_handle,
                        ui_canvas,
                        map.groups.user.zoom,
                        vec2::new(pointer_cur.x, pointer_cur.y),
                        map.groups.user.pos.x,
                        map.groups.user.pos.y,
                        offset.x,
                        offset.y,
                        parallax.x,
                        parallax.y,
                    );

                    let radius = SOUND_POINT_RADIUS;
                    if in_radius(&sound.pos, &pointer_cur, radius) {
                        // pointer is in a drag mode
                        clicked_sound_point = true;
                        let down_point = SoundPointerDownPoint::Center;
                        self.pointer_down_state = SoundPointerDownState::Point(down_point);
                        *if latest_pointer.primary_pressed() {
                            &mut self.last_active
                        } else {
                            &mut self.last_selection
                        } = Some(SoundSelection {
                            is_background,
                            group: group_index,
                            layer: layer_index,
                            sound_index: s,
                            sound: sound.clone(),
                            point: down_point,
                            cursor_in_world_pos: None,
                        });

                        break;
                    }
                }
            }
            // else check if the pointer is down now
            if !clicked_sound_point
                && latest_pointer.primary_pressed()
                && self.last_active.is_none()
            {
                let pointer_cur = vec2::new(current_pointer_pos.x, current_pointer_pos.y);
                let pos = ui_pos_to_world_pos(
                    canvas_handle,
                    ui_canvas,
                    map.groups.user.zoom,
                    vec2::new(pointer_cur.x, pointer_cur.y),
                    map.groups.user.pos.x,
                    map.groups.user.pos.y,
                    offset.x,
                    offset.y,
                    parallax.x,
                    parallax.y,
                );
                self.pointer_down_state = SoundPointerDownState::Selection(pos);
            }
            if !clicked_sound_point && latest_pointer.primary_pressed() {
                self.last_active = None;
            }
            if latest_pointer.primary_down() && self.last_active.is_some() {
                let last_active = self.last_active.as_mut().unwrap();
                if let Some(edit_sound) = layer.layer.sounds.get(last_active.sound_index) {
                    let sound = &mut last_active.sound;

                    // handle position
                    sound.pos.x = ffixed::from_num(x1);
                    sound.pos.y = ffixed::from_num(y1);

                    if *sound != *edit_sound {
                        client.execute(
                            EditorAction::ChangeSoundAttr(ActChangeSoundAttr {
                                is_background,
                                group_index,
                                layer_index,
                                old_attr: edit_sound.clone(),
                                new_attr: sound.clone(),

                                index: last_active.sound_index,
                            }),
                            Some(&format!(
                                "change-sound-attr-{is_background}-{group_index}-{layer_index}"
                            )),
                        );
                    }
                }

                last_active.cursor_in_world_pos = Some(vec2::new(x1, y1));
            }
        }
    }

    pub fn handle_brush_draw(
        &mut self,
        ui_canvas: &UiCanvasSize,
        canvas_handle: &GraphicsCanvasHandle,
        map: &EditorMap,
        latest_pointer: &egui::PointerState,
        current_pointer_pos: &egui::Pos2,
        client: &mut EditorClient,
    ) {
        let layer = map.active_layer().unwrap();
        let (offset, parallax) = layer.get_offset_and_parallax();

        // reset brush
        if latest_pointer.secondary_pressed() {
            self.brush = None;
        }
        // apply brush
        else {
            let brush = self.brush.as_ref().unwrap();

            if latest_pointer.primary_pressed() {
                let pos = current_pointer_pos;

                let pos = vec2::new(pos.x, pos.y);

                let vec2 { x, y } = ui_pos_to_world_pos(
                    canvas_handle,
                    ui_canvas,
                    map.groups.user.zoom,
                    vec2::new(pos.x, pos.y),
                    map.groups.user.pos.x,
                    map.groups.user.pos.y,
                    offset.x,
                    offset.y,
                    parallax.x,
                    parallax.y,
                );

                let mut sounds = brush.sounds.clone();
                for sound in &mut sounds {
                    sound.pos.x += ffixed::from_num(x);
                    sound.pos.y += ffixed::from_num(y);
                }

                if let Some((action, group_indentifier)) = if let EditorLayerUnionRef::Design {
                    layer: EditorLayer::Sound(layer),
                    layer_index,
                    is_background,
                    group_index,
                    ..
                } = layer
                {
                    Some((
                        EditorAction::SoundLayerAddSounds(ActSoundLayerAddSounds {
                            base: ActSoundLayerAddRemSounds {
                                is_background,
                                group_index,
                                layer_index,
                                index: layer.layer.sounds.len(),
                                sounds,
                            },
                        }),
                        format!("sound-brush design {}", layer_index),
                    ))
                } else {
                    None
                } {
                    client.execute(action, Some(&group_indentifier));
                }
            }
        }
    }

    fn render_selection(
        &self,
        ui_canvas: &UiCanvasSize,
        canvas_handle: &GraphicsCanvasHandle,
        stream_handle: &GraphicsStreamHandle,
        map: &EditorMap,
        latest_pointer: &egui::PointerState,
        current_pointer_pos: &egui::Pos2,
    ) {
        let layer = map.active_layer();
        let (offset, parallax) = if let Some(layer) = &layer {
            layer.get_offset_and_parallax()
        } else {
            Default::default()
        };
        // if pointer was already down
        if let SoundPointerDownState::Selection(pointer_down) = &self.pointer_down_state {
            if latest_pointer.primary_down() {
                let pos = current_pointer_pos;
                let pos = ui_pos_to_world_pos(
                    canvas_handle,
                    ui_canvas,
                    map.groups.user.zoom,
                    vec2::new(pos.x, pos.y),
                    map.groups.user.pos.x,
                    map.groups.user.pos.y,
                    offset.x,
                    offset.y,
                    parallax.x,
                    parallax.y,
                );
                let pos = egui::pos2(pos.x, pos.y);

                let down_pos = pointer_down;
                let down_pos = egui::pos2(down_pos.x, down_pos.y);

                let rect = egui::Rect::from_min_max(pos, down_pos);

                render_rect(
                    canvas_handle,
                    stream_handle,
                    map,
                    rect,
                    ubvec4::new(255, 0, 0, 255),
                    &parallax,
                    &offset,
                );
            }
        }
    }

    fn render_brush(
        &self,
        ui_canvas: &UiCanvasSize,
        canvas_handle: &GraphicsCanvasHandle,
        stream_handle: &GraphicsStreamHandle,
        map: &EditorMap,
        latest_pointer: &egui::PointerState,
        current_pointer_pos: &egui::Pos2,
    ) {
        let layer = map.active_layer();
        let (offset, parallax) = if let Some(layer) = &layer {
            layer.get_offset_and_parallax()
        } else {
            Default::default()
        };

        let brush = self.brush.as_ref().unwrap();

        let pos = current_pointer_pos;
        let pos_on_map = ui_pos_to_world_pos(
            canvas_handle,
            ui_canvas,
            map.groups.user.zoom,
            vec2::new(pos.x, pos.y),
            map.groups.user.pos.x,
            map.groups.user.pos.y,
            offset.x,
            offset.y,
            parallax.x,
            parallax.y,
        );
        let pos = pos_on_map;
        let pos = egui::pos2(pos.x, pos.y);

        let mut state = State::new();

        let (center, group_attr) = if self.parallax_aware_brush {
            (
                map.groups.user.pos - pos_on_map,
                layer.map(|layer| layer.get_or_fake_group_attr()),
            )
        } else {
            let pos = current_pointer_pos;
            let pos_on_map = ui_pos_to_world_pos(
                canvas_handle,
                ui_canvas,
                map.groups.user.zoom,
                vec2::new(pos.x, pos.y),
                map.groups.user.pos.x,
                map.groups.user.pos.y,
                0.0,
                0.0,
                100.0,
                100.0,
            );
            (map.groups.user.pos - pos_on_map, None)
        };
        RenderTools::map_canvas_of_group(
            canvas_handle,
            &mut state,
            center.x,
            center.y,
            group_attr.as_ref(),
            map.groups.user.zoom,
        );
        RenderMap::render_sounds(
            stream_handle,
            &map.animations,
            &map.user.time,
            &map.animation_time(),
            brush.sounds.iter(),
            state,
        );

        let brush_size = vec2::new(brush.w, brush.h);
        let rect =
            egui::Rect::from_min_max(pos, egui::pos2(pos.x + brush_size.x, pos.y + brush_size.y));

        render_rect(
            canvas_handle,
            stream_handle,
            map,
            rect,
            ubvec4::new(255, 0, 0, 255),
            &parallax,
            &offset,
        );
    }

    pub fn update(
        &mut self,
        ui_canvas: &UiCanvasSize,
        stream_handle: &GraphicsStreamHandle,
        graphics_mt: &GraphicsMultiThreaded,
        buffer_object_handle: &GraphicsBufferObjectHandle,
        backend_handle: &GraphicsBackendHandle,
        canvas_handle: &GraphicsCanvasHandle,
        map: &EditorMap,
        fake_texture: &TextureContainer,
        latest_pointer: &egui::PointerState,
        current_pointer_pos: &egui::Pos2,
        latest_modifiers: &egui::Modifiers,
        client: &mut EditorClient,
    ) {
        let layer = map.active_layer();
        if !layer.as_ref().is_some_and(|layer| layer.is_sound_layer()) {
            return;
        }

        if self.brush.is_none() || self.pointer_down_state.is_selection() {
            self.handle_brush_select(
                ui_canvas,
                graphics_mt,
                buffer_object_handle,
                backend_handle,
                canvas_handle,
                map,
                fake_texture,
                latest_pointer,
                current_pointer_pos,
                latest_modifiers,
                client,
            );
        } else {
            self.handle_brush_draw(
                ui_canvas,
                canvas_handle,
                map,
                latest_pointer,
                current_pointer_pos,
                client,
            );
        }
    }

    pub fn render(
        &mut self,
        ui_canvas: &UiCanvasSize,
        stream_handle: &GraphicsStreamHandle,
        graphics_mt: &GraphicsMultiThreaded,
        buffer_object_handle: &GraphicsBufferObjectHandle,
        backend_handle: &GraphicsBackendHandle,
        canvas_handle: &GraphicsCanvasHandle,
        map: &EditorMap,
        latest_pointer: &egui::PointerState,
        current_pointer_pos: &egui::Pos2,
        client: &mut EditorClient,
    ) {
        let layer = map.active_layer();
        if !layer.as_ref().is_some_and(|layer| layer.is_sound_layer()) {
            return;
        }

        render_sound_points(
            ui_canvas,
            layer,
            current_pointer_pos,
            stream_handle,
            canvas_handle,
            map,
        );

        if self.brush.is_none() || self.pointer_down_state.is_selection() {
            self.render_selection(
                ui_canvas,
                canvas_handle,
                stream_handle,
                map,
                latest_pointer,
                current_pointer_pos,
            );
        } else {
            self.render_brush(
                ui_canvas,
                canvas_handle,
                stream_handle,
                map,
                latest_pointer,
                current_pointer_pos,
            );
        }
    }
}
