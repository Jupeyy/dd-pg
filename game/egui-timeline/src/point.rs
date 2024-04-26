use std::{
    collections::{HashMap, HashSet},
    ops::RangeInclusive,
    time::Duration,
};

use egui::Color32;
use map::map::animations::{AnimPointColor, AnimPointPos, AnimPointSound};
use math::math::vector::{ffixed, nffixed};

/// a channel of a graph [`Point`].
/// This could for example be the R channel in a RGBA point
pub trait PointChannel {
    fn value(&self) -> f32;
    fn set_value(&mut self, val: f32);
}

impl PointChannel for ffixed {
    fn value(&self) -> f32 {
        self.to_num()
    }

    fn set_value(&mut self, val: f32) {
        *self = ffixed::from_num(val);
    }
}

impl PointChannel for nffixed {
    fn value(&self) -> f32 {
        self.to_num()
    }

    fn set_value(&mut self, val: f32) {
        *self = nffixed::from_num(val.clamp(0.0, 1.0));
    }
}

/// information about points in the graph
pub trait Point {
    /// time axis value of the point
    fn time_mut(&mut self) -> &mut Duration;
    fn time(&self) -> &Duration;
    /// e.g. for a color value this would be R, G, B(, A)
    /// (name, color, range of possible/allowed values, interface to interact with channel)
    fn channels(&mut self) -> Vec<(&str, Color32, RangeInclusive<f32>, &mut dyn PointChannel)>;
}

impl Point for AnimPointPos {
    fn time_mut(&mut self) -> &mut Duration {
        &mut self.time
    }
    fn time(&self) -> &Duration {
        &self.time
    }

    fn channels(&mut self) -> Vec<(&str, Color32, RangeInclusive<f32>, &mut dyn PointChannel)> {
        vec![
            ("x", Color32::YELLOW, f32::MIN..=f32::MAX, &mut self.value.x),
            ("y", Color32::KHAKI, f32::MIN..=f32::MAX, &mut self.value.y),
            ("r", Color32::BROWN, f32::MIN..=f32::MAX, &mut self.value.z),
        ]
    }
}

impl Point for AnimPointColor {
    fn time_mut(&mut self) -> &mut Duration {
        &mut self.time
    }
    fn time(&self) -> &Duration {
        &self.time
    }

    fn channels(&mut self) -> Vec<(&str, Color32, RangeInclusive<f32>, &mut dyn PointChannel)> {
        vec![
            ("r", Color32::RED, 0.0..=1.0, &mut self.value.x),
            ("g", Color32::GREEN, 0.0..=1.0, &mut self.value.y),
            ("b", Color32::BLUE, 0.0..=1.0, &mut self.value.z),
            ("a", Color32::GRAY, 0.0..=1.0, &mut self.value.w),
        ]
    }
}

impl Point for AnimPointSound {
    fn time_mut(&mut self) -> &mut Duration {
        &mut self.time
    }
    fn time(&self) -> &Duration {
        &self.time
    }

    fn channels(&mut self) -> Vec<(&str, Color32, RangeInclusive<f32>, &mut dyn PointChannel)> {
        vec![("v", Color32::GOLD, 0.0..=1.0, &mut self.value)]
    }
}

/// a group of points
pub struct PointGroup<'a> {
    /// the name of the point collection (e.g. "Color" or "Position")
    pub name: &'a str,
    /// an opaque type that implements [`Point`]
    pub points: Vec<&'a mut dyn Point>,
    /// timeline graph - currently selected points (e.g. by a pointer click)
    pub selected_points: &'a mut HashSet<usize>,
    /// timeline graph - currently hovered point (e.g. by a pointer)
    pub hovered_point: &'a mut Option<usize>,
    /// value graph - currently selected points + their channels (e.g. by a pointer click)
    pub selected_point_channels: &'a mut HashMap<usize, HashSet<usize>>,
}
