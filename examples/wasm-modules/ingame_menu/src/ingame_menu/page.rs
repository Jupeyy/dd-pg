use std::{path::Path, sync::Arc};

use api::IO;
use api_ui_game::render::{
    create_ctf_container, create_emoticons_container, create_entities_container,
    create_flags_container, create_freeze_container, create_game_container, create_hook_container,
    create_hud_container, create_ninja_container, create_particles_container,
    create_skin_container, create_weapon_container,
};
use client_containers::{
    ctf::CtfContainer, emoticons::EmoticonsContainer, entities::EntitiesContainer,
    flags::FlagsContainer, freezes::FreezeContainer, game::GameContainer, hooks::HookContainer,
    hud::HudContainer, ninja::NinjaContainer, particles::ParticlesContainer, skins::SkinContainer,
    weapons::WeaponContainer,
};
use client_render_base::{
    map::{
        map_buffered::{ClientMapBuffered, TileLayerVisuals},
        map_pipeline::MapGraphics,
    },
    render::{tee::RenderTee, toolkit::ToolkitRender},
};
use client_ui::{
    client_info::ClientInfo,
    events::UiEvents,
    ingame_menu::{
        account_info::AccountInfo,
        server_info::{GameInfo, GameServerInfo},
        server_players::ServerPlayers,
        votes::Votes,
    },
    main_menu::{
        monitors::UiMonitors, profiles_interface::ProfilesInterface,
        theme_container::ThemeContainer, user_data::MainMenuInterface,
    },
};
use game_config::config::Config;
use game_interface::{
    types::{character_info::NetworkCharacterInfo, id_gen::IdGenerator},
    votes::MapVote,
};
use graphics::{
    graphics::graphics::Graphics,
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::backend::GraphicsBackendHandle, canvas::canvas::GraphicsCanvasHandle,
        stream::stream::GraphicsStreamHandle, texture::texture::GraphicsTextureHandle,
    },
};
use shared_base::server_browser::{
    ServerBrowserData, ServerBrowserInfo, ServerBrowserInfoMap, ServerBrowserServer,
};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use super::{create_theme_container, profiles::Profiles};

struct MenuImpl {}

impl MainMenuInterface for MenuImpl {
    fn refresh(&mut self) {}
    fn refresh_demo_list(&mut self, _path: &Path) {}
    fn refresh_demo_info(&mut self, _file: &Path) {}
}

pub struct IngameMenu {
    config: Config,

    backend_handle: GraphicsBackendHandle,
    stream_handle: GraphicsStreamHandle,
    canvas_handle: GraphicsCanvasHandle,
    texture_handle: GraphicsTextureHandle,
    graphics_mt: GraphicsMultiThreaded,

    skin_container: SkinContainer,
    flags_container: FlagsContainer,
    render_tee: RenderTee,
    toolkit_render: ToolkitRender,
    weapons_container: WeaponContainer,
    hook_container: HookContainer,
    entities_container: EntitiesContainer,
    freeze_container: FreezeContainer,
    emoticons_container: EmoticonsContainer,
    particles_container: ParticlesContainer,
    ninja_container: NinjaContainer,
    game_container: GameContainer,
    hud_container: HudContainer,
    ctf_container: CtfContainer,
    theme_container: ThemeContainer,
    map_render: MapGraphics,
    tile_layer_visuals: TileLayerVisuals,
}

impl IngameMenu {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            config: Config::default(),

            backend_handle: graphics.backend_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            canvas_handle: graphics.canvas_handle.clone(),
            texture_handle: graphics.texture_handle.clone(),
            graphics_mt: graphics.get_graphics_mt(),

            skin_container: create_skin_container(),
            render_tee: RenderTee::new(graphics),
            flags_container: create_flags_container(),
            toolkit_render: ToolkitRender::new(graphics),
            weapons_container: create_weapon_container(),
            hook_container: create_hook_container(),
            entities_container: create_entities_container(),
            freeze_container: create_freeze_container(),
            emoticons_container: create_emoticons_container(),
            particles_container: create_particles_container(),
            ninja_container: create_ninja_container(),
            game_container: create_game_container(),
            hud_container: create_hud_container(),
            ctf_container: create_ctf_container(),
            theme_container: create_theme_container(),
            map_render: MapGraphics::new(&graphics.backend_handle),
            tile_layer_visuals: ClientMapBuffered::tile_set_preview(
                &graphics.get_graphics_mt(),
                &graphics.buffer_object_handle,
                &graphics.backend_handle,
            ),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        let mut servers = Vec::new();
        for i in 0..100 {
            servers.push(ServerBrowserServer {
                info: ServerBrowserInfo {
                    name: format!("demo_server {i}"),
                    game_type: format!("demo_server {i}"),
                    version: format!("demo_version {i}"),
                    map: ServerBrowserInfoMap {
                        name: format!("demo_server {i}"),
                        blake3: Default::default(),
                        size: 0,
                    },
                    players: Vec::new(),
                    max_players: 64,
                    passworded: false,
                    cert_sha256_fingerprint: Default::default(),
                },
                address: "127.0.0.1:8303".into(),
                location: "default".to_string(),
            });
        }
        client_ui::ingame_menu::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::ingame_menu::user_data::UserData {
                    browser_menu: client_ui::main_menu::user_data::UserData {
                        browser_data: &mut ServerBrowserData { servers },
                        ddnet_info: &Default::default(),
                        demos: &Default::default(),
                        demo_info: &None,
                        icons: &mut Default::default(),

                        server_info: &Default::default(),
                        render_options: client_ui::main_menu::user_data::RenderOptions {
                            hide_buttons_icons: true,
                        },
                        main_menu: &mut MenuImpl {},
                        config: &mut self.config,
                        events: &UiEvents::new(),
                        client_info: &ClientInfo::default(),

                        backend_handle: &self.backend_handle,
                        stream_handle: &self.stream_handle,
                        canvas_handle: &self.canvas_handle,
                        texture_handle: &self.texture_handle,
                        graphics_mt: &self.graphics_mt,

                        render_tee: &self.render_tee,
                        skin_container: &mut self.skin_container,
                        flags_container: &mut self.flags_container,
                        toolkit_render: &mut self.toolkit_render,
                        weapons_container: &mut self.weapons_container,
                        hook_container: &mut self.hook_container,
                        entities_container: &mut self.entities_container,
                        freeze_container: &mut self.freeze_container,
                        emoticons_container: &mut self.emoticons_container,
                        particles_container: &mut self.particles_container,
                        ninja_container: &mut self.ninja_container,
                        game_container: &mut self.game_container,
                        hud_container: &mut self.hud_container,
                        ctf_container: &mut self.ctf_container,
                        theme_container: &mut self.theme_container,

                        map_render: &self.map_render,
                        tile_set_preview: &self.tile_layer_visuals,

                        spatial_chat: &Default::default(),
                        player_settings_sync: &Default::default(),

                        full_rect: ui.available_rect_before_wrap(),
                        profiles: &{
                            let profiles: Arc<dyn ProfilesInterface> = Arc::new(Profiles);
                            profiles
                        },
                        profile_tasks: &mut Default::default(),
                        io: &*unsafe { IO.borrow() },
                        monitors: &UiMonitors::new(Vec::new()),
                    },
                    server_players: &{
                        let server_players = ServerPlayers::default();

                        let id_gen = IdGenerator::new();
                        server_players.fill_player_info(
                            [(id_gen.next_id(), NetworkCharacterInfo::explicit_default())]
                                .into_iter()
                                .collect(),
                        );

                        server_players
                    },
                    game_server_info: &{
                        let game_server_info = GameServerInfo::default();

                        game_server_info.fill_game_info(GameInfo {
                            map_name: "test_map".to_string(),
                        });

                        game_server_info
                    },
                    votes: &{
                        let votes = Votes::default();

                        votes.fill_map_votes(
                            [(
                                "A_Map".to_string(),
                                MapVote {
                                    name: "A_Map".try_into().unwrap(),
                                    hash: Default::default(),
                                    thumbnail_resource: false,
                                },
                            )]
                            .into_iter()
                            .collect(),
                        );

                        votes
                    },
                    account_info: &AccountInfo::default(),
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UiPageInterface<()> for IngameMenu {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, ui_state: &mut UiState) {
        self.render_impl(ui, pipe, ui_state, false)
    }

    fn unmount(&mut self) {
        self.skin_container.clear_except_default();
    }
}
