#![allow(unused_imports)]
use csv;
use reqwest;
use matroska::{
    Matroska,
    Settings::{Audio, Video},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use walkdir::WalkDir;
use select::document::Document;
use select::predicate::{Class, Attr};
use std::collections::HashMap;

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
    
    // Create a vector of all .mkv files at depth=2
    let mut directories = Vec::new();
    
    let parent_dir = "M:/";
    for entry in WalkDir::new(parent_dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|file| file.ok())
    {
        if entry.file_name().to_string_lossy().ends_with(".mkv") {
            directories.push(entry.path().to_owned());
        }
    }

    // Create CSV from all movies
    if let Err(e) = get_csv(&directories) {
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

fn get_ratings() -> HashMap<String, String> {
    let url = "https://letterboxd.com/equus497/films/";

    let req = reqwest::blocking::get(url).expect("Did not reach the server.");
    let resp = req.text().unwrap();
    let document = Document::from(resp.as_str());
    
    let page_count = document.find(Class("pagination"))
        .into_selection()
        .first()
        .unwrap()
        .text();

    let split = page_count
        .trim()
        .rfind(" ")
        .unwrap();

    let t = page_count.trim();
    let p_count  = &t[split+1..];

    let p_total: usize = p_count.parse().unwrap();

    let page_links = (1..=p_total)
        .map(|i| format!("https://letterboxd.com/equus497/films/page/{}", i))
        .collect::<Vec<_>>();
    // ! Create hashmap of {"title": "rating"}
    let mut catalogue = HashMap::new();

    for link in page_links{

        let req = reqwest::blocking::get(link).expect("Did not reach the server.");
        let resp = req.text().unwrap();
        let cur_page = Document::from(resp.as_str());

        for poster in cur_page.find(Class("poster-container")) {
            
            let raw_title = poster.find(Attr("alt", ()))
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

fn sanitize(mut str: String) -> String {
    if str.contains(":"){
        str = str.replace(":", " -");
    }
    str
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

            println!("{} » {}", old_name.display(), new_name);
            std::fs::rename(old_name, new_name)?;
        }
        println!("Successfully renamed {} folders.", new_names.len());
    } else {
        println!("Found different number of files and folders.")
    }
    Ok(())
}

fn get_csv(directories: &Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv::Writer::from_path("D:/Movies/movie_log.csv")?;
    let ratings = get_ratings();
    
    let r_titles = ratings.keys()
        .map(|x| x.as_str())
        .collect::<Vec<_>>();

    for item in directories.iter() {
        let f = std::fs::File::open(item).unwrap();
        let matroska = Matroska::open(&f).unwrap();

        // ? GENERAL METADATA
        // Title
        let file_title = item.to_str().unwrap();
        let paren1 = &file_title.find("(").unwrap();
        let title = String::from(&file_title[3..*paren1 - 1]);

        // Year
        let paren2 = &file_title.find(")").unwrap();
        let year_str = &file_title[*paren1 + 1..*paren2];
        let year = year_str.parse::<i16>().unwrap();

        // Encoder & Remux status
        let encoder = get_encoder(&file_title);
        let remux = file_title.to_lowercase().contains("remux");

        // Size    » API returns number of bytes, must be converted
        let byte_count = std::fs::metadata(item).unwrap().len();
        let size = human_readable(byte_count as f32).parse::<f32>().unwrap();

        // Duration
        let dur_secs = matroska.info.duration.unwrap();
        let duration = get_dur(dur_secs);

        // Ratings
        let get_rating = difflib::get_close_matches(&title, r_titles.to_owned(), 1, 0.8);

        let rating = match get_rating.len() {
            0 => None,
            _ => ratings.get(get_rating[0]).cloned()
        };

        // ? VIDEO METADATA
        let vid_info = &matroska.tracks[0];

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
        let aud_info = &matroska.tracks[1];

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
            encoder,
            remux,
        })?;
    }

    writer.flush()?;
    Ok(())
}

fn get_encoder(title: &str) -> Option<String> {
    let enc_list= vec!["Tigole", "FraMeSToR", "Silence", "afm72", "DDR", "Bandi", "SAMPA", "3xO", "Joy", "RARBG", "SARTRE", "PHOCiS", "TERMiNAL", "PSA", "K1tKat", "FreetheFish", "Natty", "IchtyFinger", "BeiTai", "LEGi0N", "HDH", "HANDS", "GREENOTEA", "IWFM", "FRDS", "Ritaj", "Enthwar", "t3nzin", "EDG"];

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

// ? OLD PARSING FUNCTION
// ?     Very similar to get_csv but returns vec<Movie>
// let movie_info = parse(&directories);
/*
fn parse(directories: &Vec<PathBuf>) -> Vec<Movie> {

    let mut movie_info = Vec::new();
    for item in directories.iter() {

        let f = std::fs::File::open(item).unwrap();
        let matroska = Matroska::open(&f).unwrap();

        // ? GENERAL METADATA
        // Title
        let file_title = item.to_str().unwrap();
        let paren1 = &file_title.find("(").unwrap();
        let title = &file_title[3..*paren1-1];

        // Year
        let paren2 = &file_title.find(")").unwrap();
        let year_str = &file_title[*paren1+1..*paren2];
        let year = year_str.parse::<i16>().unwrap();

        // Size    » API returns number of bytes, must be converted
        let byte_count = std::fs::metadata(item).unwrap().len();
        let size = human_readable(byte_count as f32);

        // Duration
        let dur_secs = matroska.info.duration.unwrap();
        let duration = get_dur(dur_secs);

        // ? VIDEO METADATA

        let vid_info = &matroska.tracks[0];

        // Resolution
        let res = match &vid_info.settings {
            Video(n) if n.pixel_width > 1920 => "2160p",
            Video(n) if n.pixel_width <= 1920 => "1080p",
            _ => "9999p",
        };

        // Video Codec & Bit depth
        let (v_codec, bit_depth) = match vid_info.codec_id.as_str() {
            "V_MPEGH/ISO/HEVC" => ("x265", "10bit"),
            "V_MPEG4/ISO/AVC" => ("x264", "8bit"),
            _ => ("XXXXXXXXXX", "XXXXXXXXXX")
        };

        // ? AUDIO METADATA
        // Audio codec
        let aud_info = &matroska.tracks[1];

        let a_codec = match aud_info.codec_id.as_str() {
            "A_AAC" => "AAC",
            "A_AC3" => "AC3",
            "A_EAC3" => "EAC3",
            "A_DTS" => "DTS",
            "A_TRUEHD" => "TrueHD Atmos",
            _ => "XXX",
        };

        // Audio Channels
        let channels: f32 = match &aud_info.settings {
            Audio(c) if c.channels == 8 => 7.1,
            Audio(c) if c.channels == 7 => 6.1,
            Audio(c) if c.channels == 6 => 5.1,
            Audio(c) if c.channels == 4 => 4.0,
            Audio(c) if c.channels == 2 => 2.0,
            Audio(c) if c.channels == 0 => 1.0,
            _ => 9.9,
        };

        movie_info.push(Movie {
            title, year, size, duration, res, bit_depth, v_codec, a_codec, channels,
        });
    }
    movie_info
}

*/

// ! Use to generate an example matroska type
// let f = std::fs::File::open(&directories[99]).unwrap();
// let matroska = Matroska::open(&f).unwrap();
// println!("{:?}", matroska);
