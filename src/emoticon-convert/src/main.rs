use clap::Parser;
use client_extra::emoticon_split::Emoticon06Part;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// file name of the emoticon
    file: String,
    /// output path (directory)
    output: String,
}

fn write_part(part: Emoticon06Part, output: &str, name: &str) {
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
        client_extra::emoticon_split::split_06_emoticon(img.data, img.width, img.height).unwrap();

    std::fs::create_dir_all(&args.output).unwrap();

    write_part(converted.oop, &args.output, "oop");
    write_part(converted.exclamation, &args.output, "exclamation");
    write_part(converted.hearts, &args.output, "hearts");
    write_part(converted.drop, &args.output, "drop");
    write_part(converted.dotdot, &args.output, "dotdot");
    write_part(converted.music, &args.output, "music");
    write_part(converted.sorry, &args.output, "sorry");
    write_part(converted.ghost, &args.output, "ghost");
    write_part(converted.sushi, &args.output, "sushi");
    write_part(converted.splattee, &args.output, "splattee");
    write_part(converted.deviltee, &args.output, "deviltee");
    write_part(converted.zomg, &args.output, "zomg");
    write_part(converted.zzz, &args.output, "zzz");
    write_part(converted.wtf, &args.output, "wtf");
    write_part(converted.eyes, &args.output, "eyes");
    write_part(converted.question, &args.output, "question");
}
