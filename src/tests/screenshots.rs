use std::rc::Rc;

use base::benchmark::Benchmark;
use client_containers::utils::{load_containers, RenderGameContainers};
use client_render_base::render::{tee::RenderTee, toolkit::ToolkitRender};
use graphics::graphics::graphics::Graphics;
use graphics_backend::backend::GraphicsBackend;
use ui_base::{
    font_data::{UiFontData, UiFontDataLoading},
    ui::UiCreator,
};

use super::{
    actionfeed::test_actionfeed, base::get_base, chat::test_chat, emote_wheel::test_emote_wheel,
    hud::test_hud, ingame::test_ingame, motd::test_motd, scoreboard::test_scoreboard,
    screenshot::save_screenshot, spectator_selection::test_spectator_selection, vote::test_vote,
};

fn prepare(
    backend_validation: bool,
) -> (
    Graphics,
    Rc<GraphicsBackend>,
    UiCreator,
    RenderGameContainers,
    RenderTee,
    ToolkitRender,
) {
    let (io, tp, graphics, graphics_backend, sound) = get_base(backend_validation);

    let font_loading = UiFontDataLoading::new(&io);
    let font_data = UiFontData::new(font_loading)
        .unwrap()
        .into_font_definitions();

    let mut creator = UiCreator::default();
    creator.load_font(&font_data);

    let scene = sound.scene_handle.create(Default::default());
    let containers = load_containers(&io, &tp, None, None, true, &graphics, &sound, &scene);

    let render_tee = RenderTee::new(&graphics);
    let toolkit_render = ToolkitRender::new(&graphics);

    (
        graphics,
        graphics_backend,
        creator,
        containers,
        render_tee,
        toolkit_render,
    )
}

fn test_screenshots(
    backend_validation: bool,
    save_screenshot: impl Fn(&Graphics, &Rc<GraphicsBackend>, &str),
) {
    let (graphics, graphics_backend, creator, mut containers, render_tee, toolkit_render) =
        prepare(backend_validation);

    test_hud(&graphics, &creator, &mut containers, &render_tee, |name| {
        save_screenshot(&graphics, &graphics_backend, name)
    });
    test_scoreboard(&graphics, &creator, &mut containers, &render_tee, |name| {
        save_screenshot(&graphics, &graphics_backend, name)
    });
    test_chat(&graphics, &creator, &mut containers, &render_tee, |name| {
        save_screenshot(&graphics, &graphics_backend, name)
    });
    test_actionfeed(
        &graphics,
        &creator,
        &mut containers,
        &render_tee,
        &toolkit_render,
        |name| save_screenshot(&graphics, &graphics_backend, name),
    );
    test_emote_wheel(&graphics, &creator, &mut containers, &render_tee, |name| {
        save_screenshot(&graphics, &graphics_backend, name)
    });
    test_motd(&graphics, &creator, |name| {
        save_screenshot(&graphics, &graphics_backend, name)
    });
    test_vote(&graphics, &creator, &mut containers, &render_tee, |name| {
        save_screenshot(&graphics, &graphics_backend, name)
    });
    test_spectator_selection(&graphics, &creator, &mut containers, &render_tee, |name| {
        save_screenshot(&graphics, &graphics_backend, name)
    });
    test_ingame(
        &graphics,
        &creator,
        &mut containers,
        |name| save_screenshot(&graphics, &graphics_backend, name),
        100,
        1,
    );
}

#[test]
fn create_screenshots() {
    test_screenshots(true, |graphics, graphics_backend, name| {
        save_screenshot(graphics, graphics_backend, name)
    });
}

#[test]
fn benchmark_screenshots() {
    let (graphics, _, creator, mut containers, _, _) = prepare(true);
    let b = Benchmark::new(true);
    test_ingame(
        &graphics,
        &creator,
        &mut containers,
        |_| {
            // ignore
        },
        10000,
        5,
    );
    b.bench("ingame 10000 players, 5 run(s)");
}
