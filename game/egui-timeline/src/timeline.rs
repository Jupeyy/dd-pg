use std::time::Duration;

use egui::{
    pos2, text::LayoutJob, vec2, Align2, Color32, DragValue, FontId, Key, KeyboardShortcut,
    Modifiers, Pos2, Rect, RichText, Shape, Stroke, TextFormat, Vec2,
};
use egui_extras::{Size, StripBuilder};

use crate::point::{Point, PointGroup};

#[derive(Debug, Clone, Copy)]
struct GraphProps {
    /// scale of the axes
    scale: Vec2,
    /// offset / position in graph, an offset of 0 means that 0 on x
    /// is the most left (bcs timeline can't get negative) and 0 of y is centered
    offset: Pos2,
}

#[derive(Debug, Clone, Copy)]
struct Time {
    pub time: Duration,
    /// while dragger is active, this is the smooth "real" value
    pub down_time_smooth: Duration,
}

#[derive(Debug, Clone, Copy)]
enum PointerDownState {
    None,
    Graph { pos: Pos2, scroll_y: bool },
    Time(Pos2),
    TimelinePoint(Pos2),
    ValuePoint(Pos2),
}

impl PointerDownState {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    pub fn is_graph(&self) -> bool {
        matches!(self, Self::Graph { .. })
    }
    pub fn is_time(&self) -> bool {
        matches!(self, Self::Time(_))
    }
    pub fn is_timeline_point(&self) -> bool {
        matches!(self, Self::TimelinePoint(_))
    }
    pub fn is_value_point(&self) -> bool {
        matches!(self, Self::ValuePoint(_))
    }
    pub fn as_ref(&self) -> Option<&Pos2> {
        match self {
            PointerDownState::None => None,
            PointerDownState::Graph { pos, .. }
            | PointerDownState::Time(pos)
            | PointerDownState::TimelinePoint(pos)
            | PointerDownState::ValuePoint(pos) => Some(pos),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum PlayDir {
    Paused,
    Backward,
    Forward,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct TimelineResponse {
    /// the time changed, either because the timeline is currently set to `playing`
    /// or because the user moved the time dragger
    pub time_changed: bool,
    /// the upper implementation should insert a new animation point
    /// (or replace an existing one) at the current position,
    /// if the implementation supports adding frame point data outside of this panel
    pub insert_or_replace_point: bool,
}

/// represents animation points in twmaps
#[derive(Debug, Copy, Clone)]
pub struct Timeline {
    stroke_size: f32,
    point_radius: f32,

    props: GraphProps,
    time: Time,

    pointer_down_pos: PointerDownState,
    drag_val: f32,

    play_dir: PlayDir,
    last_time: Option<f64>,
}

fn size_per_int(zoom: f32) -> f32 {
    100.0 / zoom
}

pub struct AxisValue {
    x_axis_y_off: f32,
    font_size: f32,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            stroke_size: 2.0,
            point_radius: 5.0,

            props: GraphProps {
                offset: pos2(0.0, 0.0),
                scale: vec2(1.0, 1.0),
            },
            time: Time {
                time: Duration::ZERO,
                down_time_smooth: Duration::ZERO,
            },

            pointer_down_pos: PointerDownState::None,
            drag_val: 0.0,

            play_dir: PlayDir::Paused,
            last_time: None,
        }
    }

    fn background(ui: &egui::Ui, value_graph: bool) {
        let painter = ui.painter();
        painter.rect_filled(
            ui.available_rect_before_wrap(),
            0.0,
            if value_graph {
                Color32::BLACK
            } else {
                Color32::BLACK
            },
        );
    }

    fn inner_graph_rect(&self, ui: &egui::Ui) -> Rect {
        let rect = ui.available_rect_before_wrap();
        Rect::from_min_size(
            pos2(rect.min.x + self.point_radius, rect.min.y),
            vec2(rect.width() - self.point_radius * 2.0, rect.height()),
        )
    }

    fn handle_input(&mut self, ui: &egui::Ui, is_readonly: bool, can_y_scroll: bool) {
        if is_readonly || (!self.pointer_down_pos.is_graph() && !self.pointer_down_pos.is_none()) {
            return;
        }

        let rect = ui.available_rect_before_wrap();
        ui.input(|i| {
            let pointer_pos = i.pointer.interact_pos().unwrap_or_default();
            if i.pointer.primary_down() {
                let should_scroll_y =
                    if let PointerDownState::Graph { scroll_y, .. } = &self.pointer_down_pos {
                        *scroll_y
                    } else {
                        can_y_scroll
                    };
                if let PointerDownState::Graph {
                    pos: pointer_down_pos,
                    ..
                } = &self.pointer_down_pos
                {
                    let x_diff = pointer_pos.x - pointer_down_pos.x;
                    let y_diff = pointer_pos.y - pointer_down_pos.y;
                    if should_scroll_y {
                        self.props.offset.y -= y_diff;
                    } else {
                        self.props.offset.x -= x_diff;
                        self.props.offset.x = self.props.offset.x.clamp(0.0, f32::MAX);
                    }
                }
                if (rect.contains(pointer_pos) && i.pointer.primary_pressed())
                    || self.pointer_down_pos.is_graph()
                {
                    self.pointer_down_pos = PointerDownState::Graph {
                        pos: pointer_pos,
                        scroll_y: should_scroll_y,
                    };
                }
            } else {
                self.pointer_down_pos = PointerDownState::None;
            }
            if rect.contains(pointer_pos) {
                if can_y_scroll && !i.modifiers.shift {
                    let prev_scale_y = self.props.scale.y;
                    self.props.scale.y -= i.smooth_scroll_delta.y / 100.0;
                    self.props.scale.y = self.props.scale.y.clamp(0.5, f32::MAX);

                    if prev_scale_y != self.props.scale.y {
                        let zoom_fac = self.props.scale.y / prev_scale_y;
                        self.props.offset.y /= zoom_fac;
                    }
                } else {
                    let prev_scale_x = self.props.scale.x;
                    self.props.scale.x -= i.smooth_scroll_delta.y / 100.0;
                    self.props.scale.x = self.props.scale.x.clamp(0.5, f32::MAX);

                    if prev_scale_x != self.props.scale.x {
                        let zoom_fac = self.props.scale.x / prev_scale_x;
                        self.props.offset.x /= zoom_fac;
                    }
                }
            }
        });
    }

    fn axes_value(&self, ui: &egui::Ui, as_value_graph: bool, rect: Rect) -> AxisValue {
        let font_size = 10.0;
        let y_extra = if as_value_graph {
            rect.height() / 2.0 + self.stroke_size / 2.0
        } else {
            rect.height() - self.stroke_size / 2.0 - font_size - 5.0
        };

        AxisValue {
            x_axis_y_off: y_extra,
            font_size,
        }
    }

    fn draw_axes(&self, ui: &egui::Ui, as_value_graph: bool) -> AxisValue {
        let painter = ui.painter();

        let rect = ui.available_rect_before_wrap();
        let res = self.axes_value(ui, as_value_graph, rect);
        let AxisValue {
            x_axis_y_off: y_extra,
            font_size,
        } = res;

        let rect = ui.available_rect_before_wrap();
        let x_off = rect.min.x;
        let y_off = rect.min.y + y_extra
            - if as_value_graph {
                self.props.offset.y
            } else {
                0.0
            };
        let width = rect.width();
        let height = rect.height();
        let steps_x = self.props.scale.x.round() as usize;
        let step_size_x = size_per_int(self.props.scale.x) * steps_x as f32;
        let min_x = (self.props.offset.x / step_size_x).floor() * steps_x as f32;
        let max_x = ((self.props.offset.x + width) / step_size_x).ceil() * steps_x as f32;

        if as_value_graph {
            let y_axis_size = size_per_int(self.props.scale.y).clamp(1.0, f32::MAX);

            let max_steps = (height / y_axis_size) as i32;
            let steps_upper_half_y = ((rect.max.y - y_off) / y_axis_size) as i32;
            let steps_lower_half_y = ((y_off - rect.min.y) / y_axis_size) as i32;

            for y in (-steps_lower_half_y..=steps_upper_half_y).take((max_steps.abs() + 2) as usize)
            {
                let y_off = y_off + y as f32 * y_axis_size;
                painter.line_segment(
                    [pos2(x_off, y_off), pos2(x_off + width, y_off)],
                    Stroke::new(
                        if y == 0 {
                            self.stroke_size
                        } else {
                            self.stroke_size / 4.0
                        },
                        Color32::WHITE,
                    ),
                );
            }
        } else {
            painter.line_segment(
                [pos2(x_off, y_off), pos2(x_off + width, y_off)],
                Stroke::new(self.stroke_size, Color32::WHITE),
            );
        }

        for x in (min_x.round() as i32..=max_x.round() as i32).step_by(steps_x) {
            let pos = pos2(
                x_off + (-self.props.offset.x) + (x as f32 * size_per_int(self.props.scale.x)),
                y_off + font_size,
            );
            painter.text(
                pos2(pos.x + if as_value_graph { 4.0 } else { 0.0 }, pos.y),
                if as_value_graph {
                    Align2::LEFT_CENTER
                } else {
                    Align2::CENTER_CENTER
                },
                format!("{}", x),
                egui::FontId::proportional(font_size),
                Color32::GRAY,
            );
            let y_min = if as_value_graph {
                rect.min.y
            } else {
                y_off - 3.0
            };
            let y_max = if as_value_graph {
                rect.max.y
            } else {
                y_off + 3.0
            };
            painter.line_segment(
                [pos2(pos.x, y_min), pos2(pos.x, y_max)],
                Stroke::new(self.stroke_size / 2.0, Color32::GRAY),
            );
        }

        res
    }

    fn handle_input_time(
        &mut self,
        ui: &egui::Ui,
        point_groups: &mut [PointGroup<'_>],
        is_readonly: bool,
    ) {
        if is_readonly || (!self.pointer_down_pos.is_time() && !self.pointer_down_pos.is_none()) {
            return;
        }

        ui.input(|i| {
            let inner_rect = self.inner_graph_rect(ui);
            let pointer_pos = i.pointer.interact_pos().unwrap_or_default();
            if i.pointer.primary_down() {
                if let Some(pointer_down_pos) = self.pointer_down_pos.as_ref() {
                    let mut smooth_time = self.time.down_time_smooth.as_secs_f32();

                    // if pointer inside the time value dragger and ctrl is pressed,
                    // do slow drag movement
                    let slow_drag = inner_rect.contains(pointer_pos) && i.modifiers.shift;
                    let drag_val = (pointer_pos.x - pointer_down_pos.x)
                        / size_per_int(self.props.scale.x)
                        * if slow_drag { 0.5 } else { 1.0 };
                    smooth_time += drag_val;
                    smooth_time = smooth_time.clamp(0.0, f32::MAX);
                    let mut time = smooth_time;

                    // if hovering over a point apply the point's time instead
                    for point_group in point_groups {
                        if let Some(hovered_point) = point_group
                            .hovered_point
                            .and_then(|hovered_point| point_group.points.get(hovered_point))
                        {
                            time = hovered_point.time().as_secs_f32();
                        }
                        for (p, _) in point_group.hovered_point_channel.iter() {
                            if let Some(p) = point_group.points.get(*p) {
                                time = p.time().as_secs_f32();
                            }
                        }
                    }

                    // if pointer inside the time value dragger and ctrl is pressed,
                    // snap to 100ms intervals
                    if inner_rect.contains(pointer_pos) && i.modifiers.ctrl {
                        let snap_to = 100.0 / 1000.0;
                        let frac = time.rem_euclid(snap_to);
                        time -= frac;
                    }

                    self.time.time = Duration::from_secs_f32(time);
                    self.time.down_time_smooth = Duration::from_secs_f32(smooth_time);
                }
                if (i.pointer.primary_pressed() && inner_rect.contains(pointer_pos))
                    || self.pointer_down_pos.is_time()
                {
                    if self.pointer_down_pos.is_none() {
                        // move the time dragger to where the pointer was clicked originally
                        let time = ((pointer_pos.x - inner_rect.min.x + self.props.offset.x)
                            / size_per_int(self.props.scale.x))
                        .clamp(0.0, f32::MAX);

                        self.time.time = Duration::from_secs_f32(time);
                        self.time.down_time_smooth = self.time.time;
                    }
                    self.pointer_down_pos = PointerDownState::Time(pointer_pos);
                }
            } else {
                self.pointer_down_pos = PointerDownState::None;
            }
        });
    }

    fn draw_time_tri(
        &mut self,
        ui: &egui::Ui,
        point_groups: &mut [PointGroup<'_>],
        is_readonly: bool,
    ) {
        self.handle_input_time(ui, point_groups, is_readonly);
        let painter = ui.painter();

        let inner_rect = self.inner_graph_rect(ui);
        let x_off = inner_rect.min.x;
        let y_off = inner_rect.min.y;

        let time_offset =
            (self.time.time.as_secs_f32() * size_per_int(self.props.scale.x)) - self.props.offset.x;
        let x_off = x_off + time_offset;

        painter.add(Shape::Path(egui::epaint::PathShape {
            points: vec![
                pos2(x_off - 5.0, y_off),
                pos2(x_off + 5.0, y_off),
                pos2(x_off, y_off + 10.0),
            ],
            closed: true,
            fill: Color32::RED,
            stroke: Stroke::new(5.0, Color32::TRANSPARENT),
        }));
    }

    /// the points on the timeline without y axis
    fn handle_input_timeline_points(
        &mut self,
        ui: &egui::Ui,
        is_readonly: bool,
        point_groups: &mut [PointGroup<'_>],
    ) {
        let not_point_pointer_down =
            !self.pointer_down_pos.is_timeline_point() && !self.pointer_down_pos.is_none();
        if !is_readonly {
            // check if a point was clicked on, regardless of the pointer state
            ui.input(|i| {
                let inner_rect = self.inner_graph_rect(ui);
                let pointer_pos = i.pointer.interact_pos().unwrap_or_default();
                let AxisValue {
                    x_axis_y_off: y_extra,
                    ..
                } = self.axes_value(ui, false, inner_rect);
                let y_off = inner_rect.min.y + y_extra;
                let pointer_in_point_radius = |group_index: usize, point: &dyn Point| {
                    let point_center = self.offset_of_point(point.time());

                    let center = pos2(
                        inner_rect.min.x + point_center.x,
                        y_off + point_center.y
                            - 10.0
                            - group_index as f32 * (self.point_radius * 2.0 + 5.0),
                    );

                    inner_rect.contains(center)
                        && (pointer_pos - center).length().abs() < self.point_radius
                };
                // check if any point is hovered over
                'outer: for (g, point_group) in point_groups.iter_mut().enumerate() {
                    *point_group.hovered_point = None;
                    for (p, point) in point_group.points.iter_mut().enumerate() {
                        if pointer_in_point_radius(g, *point) {
                            *point_group.hovered_point = Some(p);
                            break 'outer;
                        }
                    }
                }

                if i.pointer.primary_pressed() || i.pointer.primary_down() {
                    let mut point_hit = None;

                    if i.pointer.primary_pressed() {
                        'outer: for (g, point_group) in point_groups.iter_mut().enumerate() {
                            for (p, point) in point_group.points.iter_mut().enumerate() {
                                // check if the pointer clicked on this point
                                if pointer_in_point_radius(g, *point) {
                                    point_hit = Some((g, p));
                                    break 'outer;
                                }
                            }
                        }
                    }
                    // all kind of movements are resetted if a point was clicked
                    if let PointerDownState::TimelinePoint(pointer_down_pos) = self.pointer_down_pos
                    {
                        // if pointer is down, then move all active points
                        let diff = pointer_pos.x - pointer_down_pos.x;
                        for point_group in point_groups.iter_mut() {
                            for p in point_group.selected_points.iter() {
                                let prev_point_time = (*p > 0)
                                    .then(|| {
                                        point_group
                                            .points
                                            .get(*p - 1)
                                            .map(|prev_point| prev_point.time().as_secs_f32())
                                    })
                                    .flatten();
                                let next_point_time = point_group
                                    .points
                                    .get(*p + 1)
                                    .map(|next_point| next_point.time().as_secs_f32());

                                if let Some(point) = point_group.points.get_mut(*p) {
                                    let time = point.time_mut();
                                    let mut time_secs = time.as_secs_f32();
                                    time_secs += diff / size_per_int(self.props.scale.x);
                                    time_secs = time_secs.clamp(0.0, f32::MAX);

                                    // if not the first point in group, make sure to
                                    // not move the point before a previous point
                                    if let Some(prev_point_time) = prev_point_time {
                                        time_secs =
                                            time_secs.clamp(prev_point_time + 0.00001, f32::MAX);
                                    }
                                    // if not the last point in group, make sure to
                                    // not move the point past a next point
                                    if let Some(next_point_time) = next_point_time {
                                        time_secs = time_secs.clamp(0.0, next_point_time - 0.00001);
                                    }

                                    *time = Duration::from_secs_f32(time_secs);
                                }
                            }
                        }

                        self.pointer_down_pos = PointerDownState::TimelinePoint(pointer_pos);
                    } else if let Some((g, p)) = point_hit {
                        let had_point = point_groups[g].selected_points.contains(&p);
                        if !had_point {
                            if !i.modifiers.shift {
                                // clear all points, if shift is not hold
                                for point_group in point_groups.iter_mut() {
                                    point_group.selected_points.clear();
                                }
                            }
                            point_groups[g].selected_points.insert(p);
                            self.pointer_down_pos = PointerDownState::None;
                        } else if !not_point_pointer_down {
                            self.pointer_down_pos = PointerDownState::TimelinePoint(pointer_pos);
                        }
                    } else if i.pointer.primary_pressed() && inner_rect.contains(pointer_pos) {
                        // reset all selected points (if any)
                        for point_group in point_groups.iter_mut() {
                            point_group.selected_points.clear();
                        }
                    }
                } else if self.pointer_down_pos.is_timeline_point() {
                    self.pointer_down_pos = PointerDownState::None;
                }
            });
        }
    }

    /// the points on the value graph with y axis
    fn handle_input_value_points(
        &mut self,
        ui: &egui::Ui,
        is_readonly: bool,
        point_groups: &mut [PointGroup<'_>],
    ) {
        let not_point_pointer_down =
            !self.pointer_down_pos.is_value_point() && !self.pointer_down_pos.is_none();
        let zoom_y = size_per_int(self.props.scale.y);
        if !is_readonly {
            // check if a point was clicked on, regardless of the pointer state
            ui.input(|i| {
                let inner_rect = self.inner_graph_rect(ui);
                let pointer_pos = i.pointer.interact_pos().unwrap_or_default();
                let y_extra = inner_rect.height() / 2.0 + self.stroke_size / 2.0;
                let y_off = inner_rect.min.y + y_extra - self.props.offset.y;
                let pointer_in_point_radius = |group_index: usize, point: &mut dyn Point| {
                    let point_center = self.offset_of_point(point.time());

                    point.channels().into_iter().enumerate().find_map(
                        |(index, (_, _, _, channel))| {
                            let center = pos2(
                                inner_rect.min.x + point_center.x,
                                y_off + point_center.y - channel.value() * zoom_y,
                            );

                            (inner_rect.contains(center)
                                && (pointer_pos - center).length().abs() < self.point_radius)
                                .then_some(index)
                        },
                    )
                };
                // check if any point is hovered over
                'outer: for (g, point_group) in point_groups.iter_mut().enumerate() {
                    *point_group.hovered_point_channel = Default::default();
                    for (p, point) in point_group.points.iter_mut().enumerate() {
                        if let Some(c) = pointer_in_point_radius(g, *point) {
                            point_group
                                .hovered_point_channel
                                .entry(p)
                                .or_insert_with(Default::default)
                                .insert(c);
                            break 'outer;
                        }
                    }
                }

                if i.pointer.primary_pressed() || i.pointer.primary_down() {
                    let mut point_hit = None;

                    if i.pointer.primary_pressed() {
                        'outer: for (g, point_group) in point_groups.iter_mut().enumerate() {
                            for (p, point) in point_group.points.iter_mut().enumerate() {
                                // check if the pointer clicked on this point
                                if let Some(channel) = pointer_in_point_radius(g, *point) {
                                    point_hit = Some((g, p, channel));
                                    break 'outer;
                                }
                            }
                        }
                    }
                    // all kind of movements are resetted if a point was clicked
                    if let PointerDownState::ValuePoint(pointer_down_pos) = self.pointer_down_pos {
                        // if pointer is down, then move all active points
                        let diff = pointer_pos.y - pointer_down_pos.y;
                        for point_group in point_groups.iter_mut() {
                            for (p, c) in point_group.selected_point_channels.iter() {
                                if let Some(point) = point_group.points.get_mut(*p) {
                                    let channels = point.channels();
                                    for (_, (_, _, range, channel)) in channels
                                        .into_iter()
                                        .enumerate()
                                        .filter(|(index, _)| c.contains(index))
                                    {
                                        let mut val = channel.value();
                                        val -= diff / size_per_int(self.props.scale.y);
                                        val = val.clamp(*range.start(), *range.end());

                                        channel.set_value(val);
                                    }
                                }
                            }
                        }

                        self.pointer_down_pos = PointerDownState::ValuePoint(pointer_pos);
                    } else if let Some((g, p, channel)) = point_hit {
                        let had_point = point_groups[g].selected_point_channels.contains_key(&p);
                        if !had_point {
                            if !i.modifiers.shift {
                                // clear all points, if shift is not hold
                                for point_group in point_groups.iter_mut() {
                                    point_group.selected_point_channels.clear();
                                }
                            }
                            point_groups[g]
                                .selected_point_channels
                                .insert(p, [channel].into());
                            self.pointer_down_pos = PointerDownState::None;
                        } else if !not_point_pointer_down {
                            self.pointer_down_pos = PointerDownState::ValuePoint(pointer_pos);
                        }
                    } else if i.pointer.primary_pressed() && inner_rect.contains(pointer_pos) {
                        // reset all selected points (if any)
                        for point_group in point_groups.iter_mut() {
                            point_group.selected_point_channels.clear();
                        }
                    }
                } else if self.pointer_down_pos.is_value_point() {
                    self.pointer_down_pos = PointerDownState::None;
                }
            });
        }
    }

    fn offset_of_point(&self, point_time: &Duration) -> Pos2 {
        let time_offset =
            (point_time.as_secs_f32() * size_per_int(self.props.scale.x)) - self.props.offset.x;

        pos2(time_offset, 0.0)
    }

    fn draw_point_radius_scale(
        &mut self,
        ui: &egui::Ui,
        point_time: &Duration,
        color: Color32,
        y: f32,
        scale_radius: f32,
    ) {
        let painter = ui.painter();

        let point_center = self.offset_of_point(point_time);
        let rect = ui.available_rect_before_wrap();

        let x_off = rect.min.x + point_center.x;
        let y_off = rect.min.y + point_center.y + y;

        painter.circle_filled(pos2(x_off, y_off), self.point_radius * scale_radius, color);
    }

    fn draw_point(&mut self, ui: &egui::Ui, point_time: &Duration, color: Color32, y: f32) {
        self.draw_point_radius_scale(ui, point_time, color, y, 1.0)
    }

    fn timeline_graph(
        &mut self,
        ui: &mut egui::Ui,
        point_groups: &mut [PointGroup<'_>],
        is_readonly: bool,
    ) {
        Self::background(ui, false);
        self.handle_input_timeline_points(ui, is_readonly, point_groups);
        self.handle_input(ui, is_readonly, false);

        ui.allocate_ui_at_rect(self.inner_graph_rect(ui), |ui| {
            let width = ui.available_width();
            let AxisValue { x_axis_y_off, .. } = self.draw_axes(ui, false);

            // render points
            let zoom_x = size_per_int(self.props.scale.x);
            let time_min = self.props.offset.x / zoom_x;
            let time_range = time_min..time_min + width / zoom_x;
            for (g, points_group) in point_groups.iter_mut().enumerate() {
                for (p, point) in points_group
                    .points
                    .iter_mut()
                    .enumerate()
                    .filter(|(_, point)| time_range.contains(&point.time().as_secs_f32()))
                {
                    self.draw_point(
                        ui,
                        point.time(),
                        if points_group.selected_points.contains(&p)
                            && points_group.hovered_point.is_some_and(|index| index == p)
                        {
                            Color32::LIGHT_RED
                        } else if points_group.selected_points.contains(&p) {
                            Color32::RED
                        } else if points_group.hovered_point.is_some_and(|index| index == p) {
                            Color32::LIGHT_YELLOW
                        } else {
                            Color32::YELLOW
                        },
                        x_axis_y_off - 10.0 - g as f32 * (self.point_radius * 2.0 + 5.0),
                    );
                }
            }
        });
    }

    fn value_graph(
        &mut self,
        ui: &mut egui::Ui,
        point_groups: &mut [PointGroup<'_>],
        is_readonly: bool,
    ) {
        Self::background(ui, true);
        self.handle_input_value_points(ui, is_readonly, point_groups);
        self.handle_input(ui, is_readonly, true);

        let rect = ui.available_rect_before_wrap();
        ui.allocate_ui_at_rect(
            egui::Rect::from_min_size(
                pos2(rect.min.x + self.point_radius, rect.min.y),
                vec2(rect.width() - self.point_radius * 2.0, rect.height()),
            ),
            |ui| {
                let rect = ui.available_rect_before_wrap();
                ui.set_clip_rect(rect);
                let width = ui.available_width();

                let AxisValue {
                    x_axis_y_off: y_extra,
                    ..
                } = self.draw_axes(ui, true);

                // render points
                let zoom_x = size_per_int(self.props.scale.x);
                let zoom_y = size_per_int(self.props.scale.y);

                let y_extra = y_extra - self.props.offset.y;

                let time_min = self.props.offset.x / zoom_x;
                let time_range = time_min..time_min + width / zoom_x;
                for (g, points_group) in point_groups.iter_mut().enumerate() {
                    for (p, point) in points_group
                        .points
                        .iter_mut()
                        .enumerate()
                        .filter(|(_, point)| time_range.contains(&point.time().as_secs_f32()))
                    {
                        let point_time = *point.time();
                        let channels = point.channels();
                        for (index, (_, color, _, channel)) in channels.into_iter().enumerate() {
                            let selected = points_group
                                .selected_point_channels
                                .get(&p)
                                .is_some_and(|m| m.contains(&index));
                            let hovered = points_group
                                .hovered_point_channel
                                .get(&p)
                                .is_some_and(|m| m.contains(&index));
                            let y = y_extra - channel.value() * zoom_y;
                            if hovered || selected {
                                self.draw_point_radius_scale(
                                    ui,
                                    &point_time,
                                    if selected && hovered {
                                        Color32::LIGHT_RED
                                    } else if selected {
                                        Color32::RED
                                    } else {
                                        Color32::WHITE
                                    },
                                    y,
                                    1.2,
                                );
                            }
                            self.draw_point(ui, &point_time, color, y);
                        }
                    }
                }
            },
        );
    }

    fn render_selected_points_ui(
        &mut self,
        ui: &mut egui::Ui,
        point_groups: &mut [PointGroup<'_>],
    ) {
        enum PointSelectionMode {
            Single,
            Multi,
            None,
        }
        let mut selected_points = point_groups
            .iter()
            .enumerate()
            .flat_map(|(g, point_group)| point_group.selected_points.iter().map(move |&p| (g, p)));

        let selection_mode = match selected_points.clone().count() {
            0 => PointSelectionMode::None,
            1 => PointSelectionMode::Single,
            _ => PointSelectionMode::Multi,
        };

        match selection_mode {
            PointSelectionMode::Single => {
                let (g, p) = selected_points.next().unwrap();
                let group = &mut point_groups[g];
                if let Some(selected_point) = group.points.get_mut(p) {
                    // show every channel as seperate input box
                    for (name, color, range, channel) in selected_point.channels() {
                        let mut val = channel.value();
                        ui.label(
                            RichText::new(name)
                                .background_color(color)
                                .color(Color32::WHITE),
                        );
                        ui.add(DragValue::new(&mut val).clamp_range(range).speed(0.05));
                        channel.set_value(val);
                    }
                }
            }
            PointSelectionMode::Multi => {
                // time shifting for all selected points
                ui.label("move time of points");
                ui.add(DragValue::new(&mut self.drag_val).speed(0.1));
                if ui.button("move").clicked() {
                    let selected_points: Vec<_> = selected_points.collect();
                    for (g, p) in selected_points {
                        if let Some(point) = point_groups[g].points.get_mut(p) {
                            let time = point.time_mut();
                            let mut time_secs = time.as_secs_f32();
                            time_secs += self.drag_val / size_per_int(self.props.scale.x);
                            time_secs = time_secs.clamp(0.0, f32::MAX);
                            *time = Duration::from_secs_f32(time_secs);
                        }
                    }
                }
            }
            PointSelectionMode::None => {
                ui.label("no points selected.");
            }
        }
    }

    pub fn append_icon_font_text(job: &mut LayoutJob, ui: &egui::Ui, text: &str) {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: FontId::new(12.0, egui::FontFamily::Name("icons".into())),
                color: ui.style().visuals.text_color(),
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }

    pub fn icon_font_text(ui: &egui::Ui, text: &str) -> LayoutJob {
        let mut job = LayoutJob::default();
        Self::append_icon_font_text(&mut job, ui, text);
        job
    }

    fn controls_ui(&mut self, ui: &mut egui::Ui) {
        // add a row to play/pause/reverse the graph time
        ui.horizontal(|ui| {
            if ui.button(Self::icon_font_text(ui, "\u{f04a}")).clicked() {
                self.play_dir = PlayDir::Backward;
            }
            if ui.button(Self::icon_font_text(ui, "\u{f04d}")).clicked() {
                self.play_dir = PlayDir::Paused;
                self.time.time = Duration::ZERO;
                self.last_time = None;
            }
            if matches!(self.play_dir, PlayDir::Paused) {
                if ui.button(Self::icon_font_text(ui, "\u{f04b}")).clicked() {
                    self.play_dir = PlayDir::Forward;
                }
            } else {
                if ui.button(Self::icon_font_text(ui, "\u{f04c}")).clicked() {
                    self.play_dir = PlayDir::Paused;
                    self.last_time = None;
                }
            }
        });

        if matches!(self.play_dir, PlayDir::Forward | PlayDir::Backward) {
            let time = ui.input(|i| i.time);
            let last_time = self.last_time.unwrap_or(time);

            let diff = time - last_time;
            let cur_time = self.time.time.as_secs_f64();
            let new_time = if matches!(self.play_dir, PlayDir::Forward) {
                cur_time + diff
            } else {
                cur_time - diff
            };

            self.time.time = Duration::from_secs_f64(new_time.clamp(0.0, f32::MAX as f64));

            self.last_time = Some(time);
        }
    }

    fn render_timeline(
        &mut self,
        ui: &mut egui::Ui,
        point_groups: &mut [PointGroup<'_>],
        is_readonly: bool,
    ) {
        ui.with_layout(
            egui::Layout::top_down(egui::Align::Center)
                .with_main_justify(true)
                .with_cross_justify(true),
            |ui| {
                ui.add_space(10.0);
                let rect = ui.available_rect_before_wrap();
                ui.set_clip_rect(rect);

                // time dragger
                let width = ui.available_width();
                ui.allocate_ui_at_rect(
                    egui::Rect::from_min_size(rect.min, vec2(width, 10.0)),
                    |ui| {
                        ui.set_height(ui.available_height());
                        self.draw_time_tri(ui, point_groups, is_readonly);
                    },
                );

                let rect = ui.available_rect_before_wrap();
                let height = ui.available_height();

                // timeline graph
                let top_height = height * 1.0 / 3.0;
                ui.allocate_ui_at_rect(
                    egui::Rect::from_min_size(rect.min, vec2(width, top_height)),
                    |ui| {
                        ui.set_height(ui.available_height());
                        self.timeline_graph(ui, point_groups, is_readonly);
                    },
                );

                // value graph
                ui.add_space(10.0);
                self.value_graph(ui, point_groups, is_readonly);
            },
        );
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        point_groups: &mut [PointGroup<'_>],
        is_readonly: bool,
    ) -> TimelineResponse {
        let mut res = TimelineResponse::default();
        let res_time = self.time.time;

        ui.set_height(ui.available_height());

        // controls like play, stop etc.
        self.controls_ui(ui);

        let width = ui.available_width();
        let points_props_width = 100.0;

        StripBuilder::new(ui)
            .size(Size::exact(width - points_props_width))
            .size(Size::exact(points_props_width))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    // the graphs, time dragger etc.
                    self.render_timeline(ui, point_groups, is_readonly);
                });

                strip.cell(|ui| {
                    // properties of selected point or similar
                    self.render_selected_points_ui(ui, point_groups);
                });
            });

        if self.time.time != res_time {
            res.time_changed = true;
        }
        res.insert_or_replace_point = !res.time_changed
            && ui.input_mut(|i| {
                i.consume_shortcut(&KeyboardShortcut::new(Modifiers::default(), Key::I))
            });
        res
    }

    pub fn time(&self) -> Duration {
        self.time.time
    }
}
