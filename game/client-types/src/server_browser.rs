pub struct ServerBrowserFilter {
    pub search: String,
    pub exclude: String,
}

pub struct ServerBrowserServer {
    pub name: String,
    pub game_type: String,
    pub map: String,
    pub map_sha256: String,
    pub players: Vec<()>,
}

pub struct ServerBrowserData {
    pub servers: Vec<ServerBrowserServer>,
    pub filter: ServerBrowserFilter,
    pub cur_address: String,
}
