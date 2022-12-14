// ! Things to add
//  1) Fix up user input sections.
//  2) Refactor a lot of csv related code

use csv;
use kdam::{tqdm, BarExt};
use matroska::{
    Matroska,
    Settings::{Audio, Video},
};
use reqwest;
use select::document::Document;
use select::predicate::{Attr, Class};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct Config {
    main_path: String,
    letterboxd: String,
    enc_list: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct Movie {
    title: String,
    year: i16,
    rating: Option<String>,
    size: f32,
    duration: String,
    res: i16,
    bit_depth: String,
    v_codec: String,
    a_codec: String,
    subs: Option<String>,
    channels: String,
    encoder: Option<String>,
    remux: bool,
}

trait TupleToStrings {
    fn to_strings(&self) -> (String, String);
}

impl<T1: Copy + Into<String>, T2: Copy + Into<String>> TupleToStrings for (T1, T2) {
    fn to_strings(&self) -> (String, String) {
        (self.0.into(), self.1.into())
    }
}

fn main() {
    let timer = std::time::Instant::now();
    let config = cfg_init();
    let mut directories = Vec::new();

    // Create a vector of all .mkv files at depth=2
    for entry in WalkDir::new(&config.main_path)
        .max_depth(2)
        .into_iter()
        .filter_map(|file| file.ok())
    {
        if entry.file_name().to_string_lossy().ends_with(".mkv") {
            directories.push(entry.path().to_owned());
        }
    }

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

    match input.to_lowercase().trim().chars().next().unwrap() {
        'y' => {
            if let Err(err) = rename() {
                println!("{}", err);
                std::process::exit(1);
            }
        }
        _ => {}
    }

    println!("Press Enter to exit...");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}

fn cfg_init() -> Config {
    let data = std::fs::read_to_string("./config.toml").expect("Unable to read config file.");
    toml::from_str(&data).unwrap()
}

fn get_ratings(cfg: &Config) -> HashMap<String, String> {
    let req = reqwest::blocking::get(&cfg.letterboxd).expect("Did not reach the server.");
    let res = req.text().unwrap();
    let doc = Document::from(res.as_str());

    let pagination = doc
        .find(Class("pagination"))
        .into_selection()
        .first()
        .unwrap()
        .text();

    let split = pagination.trim().rfind(" ").unwrap();

    let last_page = pagination.trim();
    let p_count = &last_page[split + 1..];

    let p_total: usize = p_count.parse().unwrap();

    let page_links = (1..=p_total)
        .map(|i| format!("{}page/{i}", &cfg.letterboxd))
        .collect::<Vec<_>>();

    let mut catalogue = HashMap::new();

    for link in page_links {
        let req = reqwest::blocking::get(link).expect("Did not reach the server.");
        let resp = req.text().unwrap();
        let cur_page = Document::from(resp.as_str());

        for poster in cur_page.find(Class("poster-container")) {
            let raw_title = poster
                .find(Attr("alt", ()))
                .into_selection()
                .first()
                .unwrap()
                .attr("alt")
                .expect("Could not find parse letterboxd.")
                .to_string();

            let title = sanitize(raw_title);
            let rating = poster.text().trim().to_string();

            catalogue.insert(title, rating);
        }
    }
    catalogue
}

fn rename() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open("D:/Movies/movie_log.csv")?;
    let mut rdr = csv::Reader::from_reader(file);

    let mut new_names = Vec::new();

    for result in rdr.deserialize() {
        let e: Movie = result?;

        let channels = match e.channels.as_str() {
            "2.0" => String::from("stereo"),
            "1.0" => String::from("mono"),
            num => format!("{}", num),
        };

        let encoder = match e.encoder {
            Some(e) => format!(" {}", e),
            None => String::new(),
        };

        let new_name = format!(
            "{} ({}) [{}p {} {} {}-{}{}] ({} GB)",
            e.title, e.year, e.res, e.v_codec, e.bit_depth, e.a_codec, channels, encoder, e.size
        );

        new_names.push(new_name);
    }

    let paths = std::fs::read_dir("M:/").unwrap();

    if std::fs::read_dir("M:/").unwrap().count() == new_names.len() {
        for (i, path) in paths.enumerate() {
            let old_name = path.unwrap().path();
            let new_name = format!("M:/{}", new_names[i]);

            println!("{} ?? {}", old_name.display(), new_name);
            std::fs::rename(old_name, new_name)?;
        }
        println!("Successfully renamed {} folders.", new_names.len());
    } else {
        println!("Found different number of files and folders.")
    }
    Ok(())
}

fn get_csv(directories: &Vec<PathBuf>, cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv::Writer::from_path("D:/Movies/movie_log.csv")?;
    let ratings = get_ratings(&cfg);

    let r_titles = ratings.keys().map(|x| x.as_str()).collect::<Vec<_>>();

    println!("Logging titles...");

    let mut pb = tqdm!(total = directories.len(), animation = "tqdm");

    for movie in directories.iter() {
        let f = std::fs::File::open(movie).unwrap();
        let matroska = Matroska::open(&f).unwrap();

        // ? GENERAL METADATA
        // Title
        let file_title = movie.to_str().unwrap();
        let paren1 = &file_title.find("(").unwrap();
        let title = String::from(&file_title[3..*paren1 - 1]);

        // Year
        let paren2 = &file_title.find(")").unwrap();
        let year_str = &file_title[*paren1 + 1..*paren2];
        let year = year_str.parse::<i16>().unwrap();

        // Encoder & Remux status
        let encoder = get_encoder(&file_title, cfg);
        let remux = file_title.to_lowercase().contains("remux");

        // Size    ?? API returns number of bytes, must be converted
        let byte_count = std::fs::metadata(movie).unwrap().len();
        let size = human_readable(byte_count as f32).parse::<f32>().unwrap();

        // Duration
        let dur_secs = matroska.info.duration.unwrap();
        let duration = get_dur(dur_secs);

        // Ratings
        let get_rating = difflib::get_close_matches(&title, r_titles.to_owned(), 1, 0.8);

        let rating = match get_rating.len() {
            0 => None,
            _ => ratings.get(get_rating[0]).cloned(),
        };

        let tracks = &matroska.tracks;

        // ? VIDEO METADATA
        let vid_info = &tracks[0];

        // Resolution
        let res: i16 = match &vid_info.settings {
            Video(n) if n.pixel_width > 1920 => 2160,
            Video(n) if n.pixel_width <= 1920 => 1080,
            _ => panic!("Could not find resolution!"),
        };

        // Video Codec & Bit depth
        let (v_codec, bit_depth) = match vid_info.codec_id.as_str() {
            "V_MPEGH/ISO/HEVC" => ("x265", "10bit"),
            "V_MPEG4/ISO/AVC" => ("x264", "8bit"),
            _ => panic!("Could not find video codec!"),
        }
        .to_strings();

        // ? AUDIO METADATA
        // Audio codec
        let aud_info = &tracks[1];

        let a_codec = match aud_info.codec_id.as_str() {
            "A_AAC" => "AAC",
            "A_AC3" => "AC3",
            "A_EAC3" => "EAC3",
            "A_DTS" => "DTS",
            "A_TRUEHD" => "TrueHD Atmos",
            _ => "XXX",
        }
        .to_string();

        // Audio Channels
        let channels: String = match &aud_info.settings {
            Audio(c) if c.channels == 8 => "7.1",
            Audio(c) if c.channels == 7 => "6.1",
            Audio(c) if c.channels == 6 => "5.1",
            Audio(c) if c.channels == 4 => "4.0",
            Audio(c) if c.channels == 2 => "2.0",
            Audio(c) if c.channels == 0 => "1.0",
            _ => panic!("Failed to find channel info."),
        }
        .to_string();

        // Subtitles
        let mut idx = 0;
        let mut has_subs = false;
        for t in tracks {
            match t.tracktype {
                matroska::Tracktype::Subtitle => {
                    has_subs = true;
                    break;
                }
                _ => idx += 1,
            };
        }

        let subs_data = match has_subs {
            true => &tracks[idx].codec_id,
            false => "None",
        };

        let subs: Option<String> = match subs_data {
            "S_VOBSUB" => Some("VOB".to_string()),
            "S_TEXT/UTF8" => Some("SRT".to_string()),
            "S_HDMV/PGS" => Some("PGS".to_string()),
            "S_TEXT/ASS" => Some("SSA".to_string()),
            _ => None,
        };

        writer.serialize(Movie {
            title,
            year,
            rating,
            size,
            duration,
            res,
            bit_depth,
            v_codec,
            a_codec,
            channels,
            subs,
            encoder,
            remux,
        })?;

        pb.update(1);
    }

    pb.refresh();
    eprint!("\n");
    writer.flush()?;
    Ok(())
}

fn get_encoder(title: &str, cfg: &Config) -> Option<String> {
    let enc_list = &cfg.enc_list;

    for enc in enc_list {
        if title.contains(enc) {
            return Some(String::from(enc));
        }
    }
    None
}

fn get_dur(x: std::time::Duration) -> String {
    // let seconds = x.as_secs() % 60;
    let minutes = (x.as_secs() / 60) % 60;
    let hours = (x.as_secs() / 60) / 60;

    format!("{}h {:02}min", hours, minutes)
}

fn human_readable(mut bytes: f32) -> String {
    for _i in ["B", "KB", "MB", "GB"] {
        if bytes < 1024.0 {
            break;
        }
        bytes /= 1024.0;
    }
    format!("{:.2}", bytes)
}

fn sanitize(mut str: String) -> String {
    if str.contains(":") {
        str = str.replace(":", " -");
    }
    str
}
// ! Use to generate an example matroska type
// let f = std::fs::File::open(&directories[99]).unwrap();
// let matroska = Matroska::open(&f).unwrap();
// println!("{:?}", matroska);
