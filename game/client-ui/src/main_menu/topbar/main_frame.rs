use egui::{epaint::RectShape, vec2, Color32, Frame, Layout, Sense, Shape};

use graphics_types::{
    commands::{TexFlags, TexFormat},
    types::{GraphicsMemoryAllocationType, ImageFormat},
};
use image::png::load_png_image;
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};
use ui_base::{
    components::menu_top_button::{menu_top_button, menu_top_button_icon, MenuTopButtonProps},
    style::topbar_buttons,
    utils::add_horizontal_margins,
};

use crate::{
    main_menu::{communities::CommunityIcon, user_data::UserData},
    utils::render_texture_for_ui,
};

use crate::{
    events::UiEvent,
    main_menu::constants::{
        MENU_DEMO_NAME, MENU_PROFILE_NAME, MENU_QUIT_NAME, MENU_SETTINGS_NAME, MENU_UI_PAGE_QUERY,
    },
    sort::{SortDir, TableSort},
};

/// main frame. full width
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        )));
    } else {
        let current_active = pipe
            .user_data
            .config
            .path()
            .query
            .get(MENU_UI_PAGE_QUERY)
            .map(|s| {
                if s.is_empty() {
                    "".to_string()
                } else {
                    s.clone()
                }
            });

        let communities = &pipe.user_data.ddnet_info.communities;

        communities.iter().for_each(|c| {
            let icons = &mut *pipe.user_data.icons;
            let icon = icons.entry(c.id.clone()).or_insert_with(|| {
                let graphics_mt = pipe.user_data.graphics_mt.clone();
                let http = pipe.user_data.io.http.clone();
                let url = c.icon.url.clone();
                url.map(|url| {
                    CommunityIcon::Loading(Ok(pipe.user_data.io.io_batcher.spawn(async move {
                        let icon = http.download_binary_secure(url).await?.to_vec();

                        let mut img_mem = None;
                        load_png_image(&icon, |width, height, _| {
                            img_mem = Some((
                                graphics_mt.mem_alloc(GraphicsMemoryAllocationType::Texture {
                                    width,
                                    height,
                                    depth: 1,
                                    is_3d_tex: false,
                                    flags: TexFlags::empty(),
                                }),
                                width,
                                height,
                            ));
                            img_mem.as_mut().unwrap().0.as_mut_slice()
                        })?;

                        Ok(img_mem.unwrap())
                    })))
                })
                .unwrap_or_else(|| CommunityIcon::Loading(Err("icon url was None".to_string())))
            });

            match icon {
                CommunityIcon::Icon(_) => {}
                CommunityIcon::Loading(task) => {
                    if task.as_ref().is_ok_and(|task| task.is_finished()) {
                        let task = std::mem::replace(task, Err("loading failed.".to_string()));
                        let task = task.unwrap().get_storage();
                        if let Ok((mem, width, height)) = task {
                            match pipe.user_data.texture_handle.load_texture(
                                width,
                                height,
                                ImageFormat::Rgba,
                                mem,
                                TexFormat::Rgba,
                                TexFlags::empty(),
                                "icon",
                            ) {
                                Ok(tex) => {
                                    *icon = CommunityIcon::Icon(tex);
                                }
                                Err(err) => {
                                    *icon = CommunityIcon::Loading(Err(err.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        });

        Frame::default()
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
            .show(ui, |ui| {
                add_horizontal_margins(ui, |ui| {
                    ui.set_style(topbar_buttons());
                    ui.horizontal(|ui| {
                        if !pipe.user_data.render_options.hide_buttons_icons
                            && menu_top_button_icon(
                                ui,
                                MenuTopButtonProps::new(MENU_PROFILE_NAME, &current_active),
                            )
                            .clicked()
                        {
                            pipe.user_data.config.path().route_query_only_single((
                                MENU_UI_PAGE_QUERY.to_string(),
                                MENU_PROFILE_NAME.to_string(),
                            ));
                        }

                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new(
                                "Internet",
                                &(current_active.clone().or(Some("Internet".to_string()))),
                            ),
                        )
                        .clicked()
                        {
                            pipe.user_data.config.path().route_query_only_single((
                                MENU_UI_PAGE_QUERY.to_string(),
                                "Internet".to_string(),
                            ));
                        }
                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new("LAN", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data.config.path().route_query_only_single((
                                MENU_UI_PAGE_QUERY.to_string(),
                                "LAN".to_string(),
                            ));
                        }
                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new("Favorites", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data.config.path().route_query_only_single((
                                MENU_UI_PAGE_QUERY.to_string(),
                                "Favorites".to_string(),
                            ));
                        }
                        if menu_top_button(
                            ui,
                            |name, ui| {
                                let icon = pipe.user_data.icons.get(name);
                                icon.map(|icon| {
                                    let (rect, res) = ui.allocate_exact_size(
                                        vec2(120.0, ui.available_height()),
                                        Sense::click(),
                                    );
                                    if let CommunityIcon::Icon(icon) = icon {
                                        render_texture_for_ui(
                                            pipe.user_data.stream_handle,
                                            pipe.user_data.canvas_handle,
                                            icon,
                                            ui,
                                            ui_state,
                                            ui.ctx().screen_rect(),
                                            Some(rect),
                                            vec2::new(rect.center().x, rect.center().y),
                                            vec2::new(rect.width(), rect.height()),
                                        );
                                    }
                                    res
                                })
                            },
                            MenuTopButtonProps::new("ddnet", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data.config.path().route_query_only_single((
                                MENU_UI_PAGE_QUERY.to_string(),
                                "ddnet".to_string(),
                            ));
                        }
                        if menu_top_button(
                            ui,
                            |_, _| None,
                            MenuTopButtonProps::new("Communities", &current_active),
                        )
                        .clicked()
                        {
                            pipe.user_data.config.path().route_query_only_single((
                                MENU_UI_PAGE_QUERY.to_string(),
                                "Communities".to_string(),
                            ));
                        }
                        if !pipe.user_data.render_options.hide_buttons_icons {
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new(MENU_QUIT_NAME, &current_active),
                                )
                                .clicked()
                                {
                                    pipe.user_data.events.push(UiEvent::Quit);
                                }
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new(MENU_SETTINGS_NAME, &current_active),
                                )
                                .clicked()
                                {
                                    pipe.user_data.config.path().route_query_only_single((
                                        MENU_UI_PAGE_QUERY.to_string(),
                                        MENU_SETTINGS_NAME.to_string(),
                                    ));
                                }
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new(MENU_DEMO_NAME, &current_active),
                                )
                                .clicked()
                                {
                                    let mut demo_dir: String =
                                        pipe.user_data.config.storage("demo-path");
                                    if demo_dir.is_empty() {
                                        demo_dir = "demos".to_string();
                                        pipe.user_data.config.set_storage("demo-path", &demo_dir);
                                    }
                                    if pipe
                                        .user_data
                                        .config
                                        .storage_opt::<TableSort>("demo.sort")
                                        .is_none()
                                    {
                                        pipe.user_data.config.set_storage(
                                            "demo.sort",
                                            &TableSort {
                                                name: "Date".to_string(),
                                                sort_dir: SortDir::Desc,
                                            },
                                        )
                                    }
                                    pipe.user_data
                                        .main_menu
                                        .refresh_demo_list(demo_dir.as_ref());
                                    pipe.user_data.config.path().route_query_only_single((
                                        MENU_UI_PAGE_QUERY.to_string(),
                                        MENU_DEMO_NAME.to_string(),
                                    ));
                                }
                                if menu_top_button_icon(
                                    ui,
                                    MenuTopButtonProps::new("\u{f279}", &current_active),
                                )
                                .clicked()
                                {
                                    pipe.user_data.events.push(UiEvent::StartEditor)
                                }
                            });
                        }
                    });
                });
            });
    }
}
