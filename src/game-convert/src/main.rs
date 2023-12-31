#![allow(clippy::all)]

use clap::Parser;
use client_extra::game_split::Game06Part;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// file name of the game
    file: String,
    /// output path (directory)
    output: String,
}

fn write_part(part: Game06Part, output: &str, name: &str) {
    let png = image::png::save_png_image(&part.data, part.width, part.height).unwrap();
    std::fs::write(output.to_string() + "/" + name + ".png", png).unwrap();
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
        client_extra::game_split::split_06_game(img.data, img.width, img.height).unwrap();

    std::fs::create_dir_all(&(args.output.clone() + "/weapons/default/hammer")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/weapons/default/gun")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/weapons/default/shotgun")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/weapons/default/grenade")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/weapons/default/laser")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/huds/default")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/pickups/default")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/hooks/default")).unwrap();
    std::fs::create_dir_all(&(args.output.clone() + "/ctfs/default")).unwrap();

    write_part(
        converted.cursor_hammer,
        &args.output,
        "weapons/default/hammer/cursor",
    );
    write_part(
        converted.cursor_gun,
        &args.output,
        "weapons/default/gun/cursor",
    );
    write_part(
        converted.cursor_shotgun,
        &args.output,
        "weapons/default/shotgun/cursor",
    );
    write_part(
        converted.cursor_grenade,
        &args.output,
        "weapons/default/grenade/cursor",
    );
    /*write_part(
        converted.cursor_ninja,
        &args.output,
        "weapons/default/ninja/cursor",
    );*/
    write_part(
        converted.cursor_laser,
        &args.output,
        "weapons/default/laser/cursor",
    );

    write_part(
        converted.weapon_hammer,
        &args.output,
        "weapons/default/hammer/weapon",
    );
    write_part(
        converted.weapon_gun,
        &args.output,
        "weapons/default/gun/weapon",
    );
    write_part(
        converted.weapon_shotgun,
        &args.output,
        "weapons/default/shotgun/weapon",
    );
    write_part(
        converted.weapon_grenade,
        &args.output,
        "weapons/default/grenade/weapon",
    );
    /*write_part(
        converted.weapon_ninja,
        &args.output,
        "weapons/default/ninja/weapon",
    );*/
    write_part(
        converted.weapon_laser,
        &args.output,
        "weapons/default/laser/weapon",
    );

    write_part(
        converted.projectile_gun,
        &args.output,
        "weapons/default/gun/projectile0",
    );
    write_part(
        converted.projectile_shotgun,
        &args.output,
        "weapons/default/shotgun/projectile0",
    );
    write_part(
        converted.projectile_grenade,
        &args.output,
        "weapons/default/grenade/projectile0",
    );
    write_part(
        converted.projectile_laser,
        &args.output,
        "weapons/default/laser/projectile0",
    );

    converted
        .muzzle_gun
        .into_iter()
        .enumerate()
        .for_each(|(index, muzzle)| {
            write_part(
                muzzle,
                &args.output,
                &("weapons/default/gun/muzzle".to_string() + &index.to_string()),
            )
        });
    converted
        .muzzle_shotgun
        .into_iter()
        .enumerate()
        .for_each(|(index, muzzle)| {
            write_part(
                muzzle,
                &args.output,
                &("weapons/default/shotgun/muzzle".to_string() + &index.to_string()),
            )
        });
    /*converted
    .muzzle_ninja
    .into_iter()
    .enumerate()
    .for_each(|(index, muzzle)| {
        write_part(
            muzzle,
            &args.output,
            &("weapons/default/ninja/muzzle".to_string() + &index.to_string()),
        )
    });*/

    write_part(converted.flag_blue, &args.output, "ctfs/default/flag_blue");
    write_part(converted.flag_red, &args.output, "ctfs/default/flag_red");

    write_part(
        converted.hook_chain,
        &args.output,
        "hooks/default/hook_chain",
    );
    write_part(converted.hook_head, &args.output, "hooks/default/hook_head");

    write_part(converted.health_full, &args.output, "huds/default/heart");
    write_part(
        converted.health_empty,
        &args.output,
        "huds/default/heart_empty",
    );
    write_part(converted.armor_full, &args.output, "huds/default/shield");
    write_part(
        converted.armor_empty,
        &args.output,
        "huds/default/shield_empty",
    );

    write_part(
        converted.pickup_health,
        &args.output,
        "pickups/default/heart",
    );
    write_part(
        converted.pickup_armor,
        &args.output,
        "pickups/default/shield",
    );
}
