use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::{path::Path, process::Command};

use crate::Photo;

mod exiftool_date_format {
    use chrono::naive::NaiveDateTime;
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%Y:%m:%d %H:%M:%S";

    pub fn serialize<S>(date: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(ref d) = *date {
            return serializer.serialize_str(&d.format(FORMAT).to_string());
        }
        serializer.serialize_none()
        //     let s = format!("{}", date.format(FORMAT));
        //     serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        if let Some(s) = s {
            return Ok(Some(
                NaiveDateTime::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)?,
            ));
        }

        Ok(None)
        // let s = String::deserialize(deserializer)?;
        // NaiveDateTime::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Exif {
    #[serde(rename = "EXIF:DateTimeOriginal")]
    #[serde(default)]
    #[serde(with = "exiftool_date_format")]
    pub date_time_original: Option<chrono::naive::NaiveDateTime>,
    #[serde(with = "exiftool_date_format")]
    #[serde(rename = "EXIF:CreateDate")]
    #[serde(default)]
    pub create_date: Option<chrono::naive::NaiveDateTime>,
    #[serde(rename = "XMP:Album")]
    #[serde(default)]
    pub album: Option<String>,
    #[serde(rename = "XMP:OriginalFileName")]
    #[serde(default)]
    pub original_filename: Option<String>,
    #[serde(rename = "EXIF:Make")]
    #[serde(default)]
    pub make: Option<String>,
    #[serde(rename = "EXIF:Model")]
    #[serde(default)]
    pub model: Option<String>,
    #[serde(rename = "EXIF:GPSLatitude")]
    #[serde(default)]
    pub gps_latitude: Option<String>,
    #[serde(rename = "EXIF:GPSLongitude")]
    #[serde(default)]
    pub gps_longitude: Option<String>,
}

pub fn get_exif(path: &Path) -> Result<Exif> {
    let teststr = Command::new("exiftool")
        .arg("-json")
        .arg("-G")
        .arg(
            path.to_str()
                .ok_or_else(|| anyhow!("Invalid path provided"))?,
        )
        .output()?
        .stdout;
    let stdout = String::from_utf8(teststr)?;

    // println!("{}", stdout);
    let mut g: Vec<Exif> = serde_json::from_str(&stdout)?;

    Ok(g.remove(0))
}

pub fn write_exif(path: &Path, photo: &Photo) -> std::io::Result<()> {
    let mut command = &mut Command::new("exiftool");

    command = command.arg("-overwrite_original");

    if let Some(original_filename) = photo.original_filename.as_ref() {
        command = command.arg(format!("-OriginalFileName={}", original_filename));

        println!(
            "\x1b[36mVerbose (write_exif\x1b[35;1m {}\x1b[36m):\x1b[0m Adding tag 'OriginalFileName': \x1b[35;1m{}\x1b[0m",
            photo.input_path.to_string_lossy(),
            original_filename
        );
    }

    if let Some(album) = photo.exif.album.as_ref() {
        command = command.arg(format!("-album={}", album));
        println!(
            "\x1b[36mVerbose (write_exif\x1b[35;1m {}\x1b[36m):\x1b[0m Adding tag 'Album': \x1b[35;1m{}\x1b[0m",
            photo.input_path.to_string_lossy(),
            album
        );
    }
    match path.to_str() {
        Some(s) => command.arg(s).output()?,
        None => return Err(std::io::Error::from(ErrorKind::InvalidFilename)),
    };

    Ok(())
}
