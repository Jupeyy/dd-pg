use crate::{
    client_map::ClientMapImage, client_map_buffered::ClientMapBuffered, datafile::CDatafileWrapper,
    game::state::GameStateInterface,
};

use base::{config::Config, system::SystemInterface};

use graphics::graphics::Graphics;

pub trait ClientInterface {}

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

pub struct RenderPipeline<'a> {
    pub map: &'a CDatafileWrapper,
    pub map_images: &'a Vec<ClientMapImage>,
    pub buffered_map: Option<&'a ClientMapBuffered>,
    pub config: &'a Config,
    pub graphics: &'a mut Graphics,
    pub sys: &'a dyn SystemInterface,
    pub client: &'a dyn ClientInterface,
    pub game: &'a dyn GameStateInterface,
    pub camera: &'a Camera,
}

impl<'a> RenderPipeline<'a> {
    pub fn new(
        map: &'a CDatafileWrapper,
        map_images: &'a Vec<ClientMapImage>,
        buffered_map: Option<&'a ClientMapBuffered>,
        config: &'a Config,
        graphics: &'a mut Graphics,
        sys: &'a dyn SystemInterface,
        client: &'a dyn ClientInterface,
        game: &'a dyn GameStateInterface,
        camera: &'a Camera,
    ) -> RenderPipeline<'a> {
        RenderPipeline {
            map: map,
            map_images: map_images,
            buffered_map: buffered_map,
            config: config,
            graphics: graphics,
            sys: sys,
            client: client,
            game: game,
            camera: camera,
        }
    }
}
