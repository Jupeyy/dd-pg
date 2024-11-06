use std::path::{Path, PathBuf};

use clap::Parser;
use client_extra::ddrace_hud_split::DdraceHudPart;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// file name of the game
    file: PathBuf,
    /// output path (directory)
    output: PathBuf,
}

fn write_part(part: DdraceHudPart, output: &Path, name: &str) {
    let png = image::png::save_png_image(&part.data, part.width, part.height).unwrap();
    std::fs::write(output.join(format!("{name}.png")), png).unwrap();
}

fn main() {
    let args = Args::parse();

    let file = std::fs::read(args.file).unwrap();
    let mut mem: Vec<u8> = Default::default();
    let img: image::png::PngResult<'_> =
        image::png::load_png_image(&file, |width, height, bytes_per_pixel| {
            mem.resize(width * height * bytes_per_pixel, Default::default());
            &mut mem
        })
        .unwrap();
    let converted =
        client_extra::ddrace_hud_split::split_ddrace_hud(img.data, img.width, img.height).unwrap();

    std::fs::create_dir_all(args.output.join("huds/default/ddrace")).unwrap();

    write_part(converted.jump, &args.output, "huds/default/ddrace/jump");
    write_part(
        converted.jump_used,
        &args.output,
        "huds/default/ddrace/jump_used",
    );
    write_part(converted.solo, &args.output, "huds/default/ddrace/solo");
    write_part(
        converted.collision_off,
        &args.output,
        "huds/default/ddrace/collision_off",
    );
    write_part(
        converted.endless_jump,
        &args.output,
        "huds/default/ddrace/endless_jump",
    );
    write_part(
        converted.endless_hook,
        &args.output,
        "huds/default/ddrace/endless_hook",
    );
    write_part(
        converted.jetpack,
        &args.output,
        "huds/default/ddrace/jetpack",
    );

    write_part(
        converted.freeze_left,
        &args.output,
        "huds/default/ddrace/freeze_left",
    );
    write_part(
        converted.freeze_right,
        &args.output,
        "huds/default/ddrace/freeze_right",
    );
    write_part(
        converted.disabled_hook_others,
        &args.output,
        "huds/default/ddrace/disabled_hook_others",
    );
    write_part(
        converted.disabled_hammer,
        &args.output,
        "huds/default/ddrace/disabled_hammer",
    );
    write_part(
        converted.disabled_shotgun,
        &args.output,
        "huds/default/ddrace/disabled_shotgun",
    );
    write_part(
        converted.disabled_grenade,
        &args.output,
        "huds/default/ddrace/disabled_grenade",
    );
    write_part(
        converted.disabled_laser,
        &args.output,
        "huds/default/ddrace/disabled_laser",
    );
    write_part(
        converted.disabled_gun,
        &args.output,
        "huds/default/ddrace/disabled_gun",
    );

    write_part(
        converted.ninja_left,
        &args.output,
        "huds/default/ddrace/ninja_left",
    );
    write_part(
        converted.ninja_right,
        &args.output,
        "huds/default/ddrace/ninja_right",
    );
    write_part(
        converted.tele_grenade,
        &args.output,
        "huds/default/ddrace/tele_grenade",
    );
    write_part(
        converted.tele_pistol,
        &args.output,
        "huds/default/ddrace/tele_pistol",
    );
    write_part(
        converted.tele_laser,
        &args.output,
        "huds/default/ddrace/tele_laser",
    );
    write_part(
        converted.deep_frozen,
        &args.output,
        "huds/default/ddrace/deep_frozen",
    );
    write_part(
        converted.live_frozen,
        &args.output,
        "huds/default/ddrace/live_frozen",
    );

    write_part(
        converted.disabled_finish,
        &args.output,
        "huds/default/ddrace/disabled_finish",
    );
    write_part(
        converted.dummy_hammer,
        &args.output,
        "huds/default/ddrace/dummy_hammer",
    );
    write_part(
        converted.dummy_copy,
        &args.output,
        "huds/default/ddrace/dummy_copy",
    );
    write_part(
        converted.stage_locked,
        &args.output,
        "huds/default/ddrace/stage_locked",
    );
    write_part(
        converted.team0_mode,
        &args.output,
        "huds/default/ddrace/team0_mode",
    );
}
