use crate::{client_map::ClientMapImage, client_map_buffered::ClientMapBuffered};

use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::{datafile::CDatafileWrapper, types::GameTickType};

use base::system::SystemInterface;
use config::config::Config;

use graphics::graphics::Graphics;
use math::math::vector::vec2;

pub trait ClientInterface {}

pub struct Camera {
    pub pos: vec2,
    pub zoom: f32,

    /// this is used for syncronized envelopes
    /// usually it should be set to the value the local player
    /// has or the player the local player is spectating
    pub animation_start_tick: GameTickType,
}

pub struct RenderPipeline<'a> {
    pub map: &'a CDatafileWrapper,
    pub map_images: &'a Vec<ClientMapImage>,
    pub buffered_map: Option<&'a ClientMapBuffered>,
    pub config: &'a Config,
    pub graphics: &'a mut Graphics,
    pub sys: &'a dyn SystemInterface,
    pub client: &'a dyn ClientInterface,
    pub game: &'a GameStateWasmManager,
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
        game: &'a GameStateWasmManager,
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
