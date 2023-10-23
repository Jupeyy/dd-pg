pub enum ScoreboardGameType {
    /// team = vanilla team
    TeamPlay {
        red_players: Vec<()>,
        blue_players: Vec<()>,
        spectator_players: Vec<()>,
    },
    SoloPlay {
        players: Vec<()>,
        spectator_players: Vec<()>,
    },
}
