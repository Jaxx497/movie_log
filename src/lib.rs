use crc;
use csv;
use reqwest;
use select::{
    document::Document,
    predicate::{Attr, Class},
};
use matroska::{
    Matroska,
    Settings::{Audio, Video},
    Track,
    Tracktype::Subtitle,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub letterboxd: String,
    pub enc_list: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub struct Movie {
    pub title: String,
    pub year: i16,
    pub rating: Option<String>,
    pub size: f32,
    pub duration: String,
    pub res: i16,
    pub bit_depth: String,
    pub v_codec: String,
    pub a_codec: String,
    pub subs: Option<String>,
    pub channels: String,
    pub encoder: Option<String>,
    pub remux: bool,
    pub hash: String,
}

trait TupleToStrings {
    fn to_strings(&self) -> (String, String);
}

impl<T1: Copy + Into<String>, T2: Copy + Into<String>> TupleToStrings for (T1, T2) {
    fn to_strings(&self) -> (String, String) {
        (self.0.into(), self.1.into())
    }
}

pub fn get_csv(directories: &Vec<PathBuf>, cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut existing_table: HashMap<String, Movie> = std::collections::HashMap::new();

    if let Ok(file) = std::fs::File::open("D:/Movies/movie_log.csv") {
        let mut rdr = csv::Reader::from_reader(file);
        for line in rdr.deserialize() {
            let mov: Movie = line.unwrap();
            existing_table.insert(mov.hash.clone(), mov);
        }
    }

    let ratings = get_ratings(&cfg);
    let r_titles = ratings.keys().map(|x| x.as_str()).collect::<Vec<_>>();

    let mut all_movies: Vec<Movie> = Vec::new();
    let mut addition = Vec::new();
    let mut existing = 0;

    for movie in directories {
        let metadata = std::fs::metadata(movie).expect("Unable to retrieve metadata.");
        let (size, hash) = get_hash(&metadata);

        if existing_table.contains_key(&hash) {
            let z = existing_table.get(&hash).unwrap();
            // println!("Existing data found for: {}", z.title);
            all_movies.push(z.clone());

            existing_table.remove(&hash);

            existing += 1;
            continue;
        }

        // ? GENERAL METADATA

        let file = std::fs::File::open(movie).expect("Could not open file.");

        let file_title = movie
            .to_str()
            .expect("Could not turn file name into string.");

        let matroska = Matroska::open(&file).expect("Could not open as matroska.");
        let tracks = &matroska.tracks;

        let dur_secs = matroska
            .info
            .duration
            .expect("Unable to retrieve duration from matroska crate.");

        let (title, year) = get_title_year(file_title);

        let encoder = get_encoder(&file_title, cfg);
        let remux = file_title.to_lowercase().contains("remux");
        let duration = get_dur(dur_secs);

        // ? Ratings

        let rating_list = difflib::get_close_matches(&title, r_titles.to_owned(), 1, 0.8);
        let rating = match rating_list.len() {
            0 => None,
            _ => ratings.get(rating_list[0]).cloned(),
        };

        let (res, v_codec, bit_depth, a_codec, channels, subs) = parse_tracks(tracks);

        addition.push(title.clone());

        all_movies.push(Movie {
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
            hash,
        });
    }

    let mut writer = csv::Writer::from_path("D:/Movies/movie_log.csv")?;

    for m in all_movies {
        writer.serialize(m)?;
    }

    eprint!("\n");
    writer.flush()?;

    println!("{:<20}{}", "Added: ", addition.len());
    for i in addition {
        println!("\t{}", i);
    }
    println!("{:<20}{}", "Removed: ", existing_table.len());
    for i in existing_table.keys() {
        let z = existing_table.get(i).unwrap();
        println!("\t{}", z.title);
    }
    println!("{:<20}{existing}", "Unchanged: ");

    Ok(())
}

fn get_ratings(cfg: &Config) -> HashMap<String, String> {
    let req = reqwest::blocking::get(&cfg.letterboxd).expect("Did not reach the server.");
    let res = req
        .text()
        .expect("Could not get a response from the server.");
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
        let resp = req
            .text()
            .expect("Retrieved the page but could not turn it into text.");
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

fn get_title_year(file_title: &str) -> (String, i16) {
    let paren1 = &file_title
        .find("(")
        .expect("Could not find parentheses in file name.");
    let paren2 = &file_title
        .find(")")
        .expect("Could not find parentheses in file name.");

    let title = String::from(&file_title[3..*paren1 - 1]);
    let year = file_title[*paren1 + 1..*paren2]
        .parse::<i16>()
        .expect("Could not parse year.");

    (title, year)
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

fn human_readable_bytes(mut bytes: f32) -> String {
    for _ in ["B", "KB", "MB", "GB"] {
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

pub const CASTAGNOLI: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

fn get_hash(metadata: &Metadata) -> (f32, String) {
    let bytes = metadata.len();

    println!("{}, {:x}", bytes, bytes);

    let last_mod = metadata
        .modified()
        .expect("Could not find any metadata on last modification.")
        .duration_since(UNIX_EPOCH)
        .expect("Could not create value from UNIX_EPOCH timestamp.")
        .as_nanos();

    let hasher = last_mod + bytes as u128;
    let digest = CASTAGNOLI.checksum(&hasher.to_ne_bytes());

    let size = human_readable_bytes(bytes as f32)
        .parse::<f32>()
        .expect("Failed to parse bytes to readable format.");

    (size, format!("{:x}", digest))
    // (size, format!("{:x}", md5::compute(hasher.to_ne_bytes())))
}

fn parse_tracks(tracks: &Vec<Track>) -> (i16, String, String, String, String, Option<String>) {
    let (res, v_codec, bit_depth) = parse_vid_info(&tracks[0]);
    let (a_codec, channels) = parse_aud_info(&tracks[1]);
    let subs = parse_first_sub(tracks);

    (res, v_codec, bit_depth, a_codec, channels, subs)
}

fn parse_vid_info(vid_meta: &Track) -> (i16, String, String) {
    let res: i16 = match &vid_meta.settings {
        Video(n) if n.pixel_width > 1920 => 2160,
        Video(n) if n.pixel_width <= 1920 => 1080,
        _ => panic!("Could not find resolution!"),
    };

    let (v_codec, bit_depth) = match vid_meta.codec_id.as_str() {
        "V_MPEGH/ISO/HEVC" => ("x265", "10bit"),
        "V_MPEG4/ISO/AVC" => ("x264", "8bit"),
        _ => panic!("Could not find video codec!"),
    }
    .to_strings();

    (res, v_codec, bit_depth)
}

fn parse_aud_info(aud_meta: &Track) -> (String, String) {
    let a_codec = match aud_meta.codec_id.as_str() {
        "A_AAC" => "AAC",
        "A_AC3" => "AC3",
        "A_EAC3" => "EAC3",
        "A_DTS" => "DTS",
        "A_TRUEHD" => "TrueHD Atmos",
        _ => "XXX",
    };

    let channels = match &aud_meta.settings {
        Audio(c) if c.channels == 8 => "7.1",
        Audio(c) if c.channels == 7 => "6.1",
        Audio(c) if c.channels == 6 => "5.1",
        Audio(c) if c.channels == 4 => "4.0",
        Audio(c) if c.channels == 2 => "2.0",
        Audio(c) if c.channels == 0 => "1.0",
        _ => panic!("Failed to find channel info."),
    };
    (a_codec, channels).to_strings()
}

fn parse_first_sub(tracks: &Vec<Track>) -> Option<String> {
    let mut subs_data = "";
    for t in tracks {
        if t.tracktype == Subtitle {
            subs_data = &t.codec_id;
            break;
        }
    }

    match subs_data {
        "S_VOBSUB" => Some(".VOB".to_string()),
        "S_TEXT/UTF8" => Some(".SRT".to_string()),
        "S_HDMV/PGS" => Some(".PGS".to_string()),
        "S_TEXT/ASS" => Some(".SSA".to_string()),
        _ => None,
    }
}

pub fn ent_to_exit() {
    println!("Press Enter to exit...");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}
