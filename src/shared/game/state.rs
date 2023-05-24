use std::time::Duration;

use base::system::SystemInterface;

use crate::types::GameTickType;

use super::{
    simulation_pipe::{SimulationPipe, SimulationPipeStage},
    stage::GameStage,
    GameElementGenerator, TGameElementID,
};

pub trait GameStateInterface {
    fn game_tick_speed(&self) -> GameTickType;
    fn game_tick(&self) -> GameTickType;
    fn prev_game_tick(&self) -> GameTickType;
    fn game_start_tick(&self) -> GameTickType;
    fn intra_tick(&self, system: &dyn SystemInterface) -> f64;
}

/**
* A game state is a collection of game related attributes such as the world, which handles
* the entities.
* the current tick, the starting tick, if the game is paused.
* the stages of the game and much more.
*/
pub struct GameState {
    stages: Vec<GameStage>,

    cur_tick: GameTickType,
    prev_tick: GameTickType,

    pub cur_monotonic_tick: u64,

    start_tick: GameTickType,
    start_tick_time: Duration,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            cur_tick: 1,
            prev_tick: 0,
            cur_monotonic_tick: 0,
            start_tick: 1,
            start_tick_time: Duration::from_nanos(0),
            stages: Vec::new(),
        }
    }

    pub fn get_stage_mut(&mut self, index: usize) -> &mut GameStage {
        &mut self.stages[index]
    }

    pub fn get_stage(&self, index: usize) -> &GameStage {
        &self.stages[index]
    }

    pub fn get_stage_by_game_el_id_mut(&mut self, id: &TGameElementID) -> Option<&mut GameStage> {
        self.stages.iter_mut().find(|s| s.game_element_id == *id)
    }

    pub fn get_stage_by_game_el_id(&self, id: &TGameElementID) -> Option<&GameStage> {
        self.stages.iter().find(|s| s.game_element_id == *id)
    }

    pub fn get_stages(&self) -> &Vec<GameStage> {
        &self.stages
    }

    pub fn get_stages_mut(&mut self) -> &mut Vec<GameStage> {
        &mut self.stages
    }

    pub fn add_stage(&mut self, GameIDGen: &mut GameElementGenerator) -> usize {
        let stage_index = self.stages.len();
        self.stages
            .push(GameStage::new(stage_index as u32, GameIDGen.get_stage_id()));
        stage_index
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    fn tick_impl(&mut self, is_prediction: bool, pipe: &mut SimulationPipe) {
        let mut sim_pipe = SimulationPipeStage::new(
            0,
            if is_prediction { 1 } else { 0 },
            pipe.player_inputs,
            is_prediction,
            pipe.collision,
        );
        for stage in &mut self.stages {
            stage.tick(&mut sim_pipe);
        }

        if !is_prediction {
            self.cur_tick += 1;
        }
    }

    pub fn tick(&mut self, pipe: &mut SimulationPipe) {
        self.tick_impl(false, pipe);

        self.cur_monotonic_tick += 1;
    }

    pub fn pred_tick(&mut self, pipe: &mut SimulationPipe) {
        self.tick_impl(true, pipe);
    }
}

impl GameStateInterface for GameState {
    fn game_tick_speed(&self) -> GameTickType {
        50
    }

    fn game_tick(&self) -> GameTickType {
        self.cur_tick
    }

    fn prev_game_tick(&self) -> GameTickType {
        self.prev_tick
    }

    fn game_start_tick(&self) -> GameTickType {
        self.start_tick
    }

    fn intra_tick(&self, system: &dyn SystemInterface) -> f64 {
        // check how much time passed since the start
        // the total passed time since the start - the time passed by amount of ticks gives the current time in the tick
        // now use this time and devide it be the amount of time that is passed per tick
        let time_per_tick = Duration::from_secs(1).as_nanos() as u64 / self.game_tick_speed();
        ((system.time_get_nanoseconds().as_nanos() - self.start_tick_time.as_nanos()) as f64
            - ((self.game_tick() - self.game_start_tick()) * time_per_tick) as f64)
            / time_per_tick as f64
    }
}
