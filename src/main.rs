use csv;
use serde::Serialize;
use std::path::PathBuf;
use walkdir::WalkDir;
use matroska::{Matroska, Settings::{Video, Audio}};

#[derive(Debug, Serialize)]
pub struct Movie <'a> {
    title: &'a str,
    year: i16,
    size: String,
    duration: String,
    res: &'a str,
    bit_depth: &'a str,
    v_codec: &'a str,
    a_codec: &'a str,
    channels: f32,
    encoder: &'a str,
    remux: bool,
}

fn main() {

    let start = std::time::Instant::now();
    
    // Create a vector of all .mkv files at depth=2
    let mut directories = Vec::new();

    for entry in WalkDir::new("M:/") 
    .max_depth(2)
    .into_iter()
    .filter_map(|file| file.ok()) {
        
        if entry.file_name().to_string_lossy().ends_with(".mkv") {
            directories.push(entry.path().to_owned());
        }
    }

    // Create CSV from all movies
    if let Err(e) = get_csv(&directories) {
        eprintln!("{}", e)
    }

    let total_time = start.elapsed();
    println!("\nSuccessfully logged {} movies and exported to movie_log.csv in {:.4?}.", directories.len(), total_time);

    // ! Use to generate an example matroska type
    // let f = std::fs::File::open(&directories[99]).unwrap();
    // let matroska = Matroska::open(&f).unwrap();
    // println!("{:?}", matroska);

    // Press Enter to quit
    println!("Press Enter to exit...");
    std::io::stdin().read_line(&mut String::new()).unwrap();

}

fn get_csv(directories: &Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {

    let mut writer = csv::Writer::from_path("D:/Movies/movie_log.csv")?;
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

        //Encoder & Remux
        let has_encoder = get_encoder(&file_title);
        let encoder = match has_encoder {
            Some(enc) => enc,
            None => "None"
        };

        let remux = file_title.to_lowercase().contains("remux");
        println!("{}", remux);

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

        writer.serialize(Movie {
            title, year, size, duration, res, bit_depth, v_codec, a_codec, channels, encoder, remux
        })?;

        // println!("Successfully finished logging {}", title);
    }

    writer.flush()?;

    Ok(())
}

fn get_encoder(title: &str) -> Option<&str> {
    let enc_list = ["Tigole", "FraMeSToR", "Silence", "afm72", "DDR", "Bandi", "SAMPA", "3xO", "Joy", "RARBG", "SARTRE", "PHOCiS", "TERMiNAL", "PSA", "K1tKat", "FreetheFish", "Natty", "IchtyFinger", "BeiTai", "LEGi0N", "HDH", "HANDS", "GREENOTEA", "IWFM", "FRDS", "Ritaj", "Enthwar", "t3nzin", "EDG"];

    for enc in enc_list{
        if title.contains(enc){
            return Some(enc);
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
            break
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
