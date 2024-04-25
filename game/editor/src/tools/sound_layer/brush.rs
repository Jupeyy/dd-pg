use client_render_base::map::render_tools::RenderTools;
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
use math::math::{
    length,
    vector::{ffixed, fvec2, ubvec4, vec2},
};

use crate::{
    actions::actions::{
        ActChangeSoundAttr, ActSoundLayerAddRemSounds, ActSoundLayerAddSounds, EditorAction,
    },
    client::EditorClient,
    map::{EditorLayer, EditorLayerUnionRef, EditorMap, EditorMapInterface},
    tools::utils::render_rect,
    utils::ui_pos_to_world_pos,
};

pub const SOUND_POINT_RADIUS: f32 = 0.75;

#[derive(Debug, Hiarc, Clone, Copy)]
pub enum SoundPointerDownPoint {
    Center,
    Corner(usize),
}

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
                let points: [fvec2; 5] = todo!("animated points by sound shape");

                if super::super::quad_layer::shared::in_box(&points[0], x0, y0, x1, y1)
                    || super::super::quad_layer::shared::in_box(&points[1], x0, y0, x1, y1)
                    || super::super::quad_layer::shared::in_box(&points[2], x0, y0, x1, y1)
                    || super::super::quad_layer::shared::in_box(&points[3], x0, y0, x1, y1)
                    || super::super::quad_layer::shared::in_box(&points[4], x0, y0, x1, y1)
                {
                    sounds.push(sound.clone());
                }
            }

            // if there is an selection, apply that
            if !sounds.is_empty() {
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
            if latest_pointer.primary_pressed() || latest_pointer.secondary_clicked() {
                for (q, sound) in layer.layer.sounds.iter().enumerate() {
                    let points: [fvec2; 5] = todo!("animated points by sound shape");

                    let pointer_cur = vec2::new(current_pointer_pos.x, current_pointer_pos.y);

                    let pointer_cur = ui_pos_to_world_pos(
                        canvas_handle,
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
                    let mut p = [false; 5];
                    p.iter_mut().enumerate().for_each(|(index, p)| {
                        *p = super::super::quad_layer::shared::in_radius(
                            &points[index],
                            &pointer_cur,
                            radius,
                        )
                    });
                    if let Some((index, _)) = p.iter().enumerate().find(|(_, &p)| p) {
                        // pointer is in a drag mode
                        clicked_sound_point = true;
                        let down_point = if index == 4 {
                            SoundPointerDownPoint::Center
                        } else {
                            SoundPointerDownPoint::Corner(index)
                        };
                        self.pointer_down_state = SoundPointerDownState::Point(down_point);
                        *if latest_pointer.primary_pressed() {
                            &mut self.last_active
                        } else {
                            &mut self.last_selection
                        } = Some(SoundSelection {
                            is_background,
                            group: group_index,
                            layer: layer_index,
                            sound_index: q,
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
                    let p = match last_active.point {
                        SoundPointerDownPoint::Center => 4,
                        SoundPointerDownPoint::Corner(index) => index,
                    };

                    let sound = &mut last_active.sound;

                    if matches!(last_active.point, SoundPointerDownPoint::Center)
                        && latest_modifiers.ctrl
                    {
                        if let Some(cursor_pos) = &last_active.cursor_in_world_pos {
                            // handle rotation
                            let diff = length(
                                &(vec2::new(x1, y1) - vec2::new(cursor_pos.x, cursor_pos.y)),
                            );

                            todo!("rotate sound?");
                        }
                    } else {
                        // handle position
                        let old_x = sound.pos.x;
                        let old_y = sound.pos.y;
                        sound.pos.x = ffixed::from_num(x1);
                        sound.pos.y = ffixed::from_num(y1);
                    }

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
        canvas_handle: &GraphicsCanvasHandle,
        map: &EditorMap,
        latest_pointer: &egui::PointerState,
        current_pointer_pos: &egui::Pos2,
        client: &mut EditorClient,
    ) {
        let layer = map.active_layer().unwrap();
        let (offset, parallax) = layer.get_offset_and_parallax();

        // reset brush
        if latest_pointer.secondary_clicked() {
            self.brush = None;
        }
        // apply brush
        else {
            let brush = self.brush.as_ref().unwrap();

            if latest_pointer.primary_down() {
                let pos = current_pointer_pos;

                let pos = vec2::new(pos.x, pos.y);

                let vec2 { x, y } = ui_pos_to_world_pos(
                    canvas_handle,
                    map.groups.user.zoom,
                    vec2::new(pos.x, pos.y),
                    map.groups.user.pos.x,
                    map.groups.user.pos.y,
                    offset.x,
                    offset.y,
                    parallax.x,
                    parallax.y,
                );

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
                                sounds: brush.sounds.clone(),
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
        todo!("render sound layer");

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

        todo!("render sound points");

        if self.brush.is_none() || self.pointer_down_state.is_selection() {
            self.render_selection(
                canvas_handle,
                stream_handle,
                map,
                latest_pointer,
                current_pointer_pos,
            );
        } else {
            self.render_brush(
                canvas_handle,
                stream_handle,
                map,
                latest_pointer,
                current_pointer_pos,
            );
        }
    }
}
