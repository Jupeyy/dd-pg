use std::time::Duration;

use egui::Rect;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};
use math::math::vector::ffixed;

#[derive(Debug, Clone)]
pub struct DemoViewerEventExport {
    pub left: Duration,
    pub right: Duration,
    pub name: String,
    pub remove_chat: bool,
}

#[derive(Debug, Clone)]
pub enum DemoViewerEvent {
    ResumeToggle,

    Stop,
    BackwardFast,
    ForwardFast,

    BackwardStep,
    ForwardStep,

    Backward,
    Forward,

    SpeedSlower,
    SpeedFaster,
    SpeedReset,

    Export(DemoViewerEventExport),

    SkipTo { time: Duration },
    PreviewAt { rect: Rect, time: Duration },

    Close,
}

#[derive(Debug, Default)]
pub struct DemoViewerUiState {
    pub pointer_on_timeline: bool,

    pub left: Option<Duration>,
    pub right: Option<Duration>,
    pub export: Option<DemoViewerEventExport>,
}

pub struct UserData<'a> {
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,

    pub is_paused: &'a bool,
    pub cur_duration: &'a Duration,
    pub max_duration: &'a Duration,
    pub speed: &'a ffixed,
    pub name: &'a str,

    pub events: &'a mut Vec<DemoViewerEvent>,

    pub state: &'a mut DemoViewerUiState,
}
