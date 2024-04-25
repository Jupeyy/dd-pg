use std::collections::VecDeque;

use client_types::actionfeed::ActionFeed;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};

pub struct UserData<'a> {
    pub entries: &'a VecDeque<ActionFeed>,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub canvas_handle: &'a GraphicsCanvasHandle,
}
