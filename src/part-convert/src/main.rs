use clap::Parser;
use client_extra::particles_split::Particles06Part;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// file name of the particles
    file: String,
    /// output path (directory)
    output: String,
}

fn write_part(part: Particles06Part, output: &str, name: &str) {
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
        client_extra::particles_split::split_06_particles(img.data, img.width, img.height).unwrap();

    std::fs::create_dir_all(args.output.clone()).unwrap();

    write_part(converted.slice, &args.output, "slice");
    write_part(converted.ball, &args.output, "ball");

    converted
        .splat
        .into_iter()
        .enumerate()
        .for_each(|(index, splat)| {
            write_part(
                splat,
                &args.output,
                &("splat".to_string() + &index.to_string()),
            )
        });

    write_part(converted.smoke, &args.output, "smoke");
    write_part(converted.shell, &args.output, "shell");

    converted
        .explosion
        .into_iter()
        .enumerate()
        .for_each(|(index, explosion)| {
            write_part(
                explosion,
                &args.output,
                &("explosion".to_string() + &index.to_string()),
            )
        });

    write_part(converted.airjump, &args.output, "airjump");

    converted
        .hit
        .into_iter()
        .enumerate()
        .for_each(|(index, hit)| {
            write_part(hit, &args.output, &("hit".to_string() + &index.to_string()))
        });
}
