// ! Things to add
//  1) Fix up user input sections.

use csv;
use movie_log::{ent_to_exit, get_csv, Config, Movie};
use std::path::PathBuf;
use walkdir::WalkDir;

fn main() {
    let timer = std::time::Instant::now();
    let config = cfg_init();
    let directories = get_dirs();

    // Create CSV from all movies
    if let Err(e) = get_csv(&directories, &config) {
        eprintln!("{}", e)
    }

    let total_time = timer.elapsed();
    println!(
        "\nSuccessfully logged {} movies and exported to movie_log.csv in {:.4?}.",
        directories.len(),
        total_time
    );

    let mut input = String::new();
    println!("Would you like to rename the folders? (Y/N)");
    std::io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");

    if let Some('y') = input.to_lowercase().trim().chars().next() {
        if let Err(err) = rename() {
            println!("{}", err);
            std::process::exit(1);
        }
    }
    
    ent_to_exit()
}

fn cfg_init() -> Config {
    let data =
        std::fs::read_to_string("D:/Movies/config.toml").expect("Unable to read config file.");
    toml::from_str(&data).unwrap()
}

fn rename() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open("D:/Movies/movie_log.csv")?;
    let mut rdr = csv::Reader::from_reader(file);

    let mut new_names = Vec::new();

    for result in rdr.deserialize() {
        let mov: Movie = result?;

        let channels = match mov.channels.as_str() {
            "2.0" => String::from("stereo"),
            "1.0" => String::from("mono"),
            num => format!("{}", num),
        };

        let encoder = match mov.encoder {
            Some(enc) => format!(" {}", enc),
            None => String::new(),
        };

        let new_name = format!(
            "{} ({}) [{}p {} {} {}-{}{}] ({} GB)",
            mov.title,
            mov.year,
            mov.res,
            mov.v_codec,
            mov.bit_depth,
            mov.a_codec,
            channels,
            encoder,
            mov.size
        );

        new_names.push(new_name);
    }

    let paths = std::fs::read_dir("M:/").unwrap();
    let mut renamed = 0;

    if std::fs::read_dir("M:/").unwrap().count() == new_names.len() {
        for (i, path) in paths.enumerate() {
            let old_name = path.unwrap().path();
            let new_name = format!("M:/{}", new_names[i]);

            if old_name.to_string_lossy() == new_name {
                continue;
            }

            renamed += 1;

            println!("{}\n\t» {}\n", old_name.display(), new_name);
            std::fs::rename(old_name, new_name)?;
        }
        println!("Successfully renamed {} folders.", renamed);
    } else {
        println!("Found different number of files and folders.")
    }
    Ok(())
}

fn get_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new("M:/")
        .max_depth(2)
        .into_iter()
        .filter_map(|file| file.ok())
    {
        if entry.file_name().to_string_lossy().ends_with(".mkv") {
            dirs.push(entry.path().to_owned());
        }
    }

    dirs
}

// ! Use to generate an example matroska type
// let f = std::fs::File::open(&directories[99]).unwrap();
// let matroska = Matroska::open(&f).unwrap();
// println!("{:?}", matroska);
