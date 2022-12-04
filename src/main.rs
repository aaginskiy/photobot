#![feature(path_file_prefix)]
#![feature(fs_try_exists)]
#![feature(io_error_more)]
#![feature(result_option_inspect)]
mod exif;
mod photohashdb;

use adler32::adler32;
use anyhow::{anyhow, Result};
use clap::Parser;
use exif::{get_exif, write_exif, Exif};
use globset::{Glob, GlobMatcher};
use once_cell::sync::{Lazy, OnceCell};
use photohashdb::load_db;
use pickledb::PickleDb;
use std::fs::{copy, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::{self};
use walkdir::WalkDir;

static GLOB_MATCHER: Lazy<GlobMatcher> =
    Lazy::new(|| Glob::new("**/*.{jpg,jpeg}").unwrap().compile_matcher());

static PHOTOHASH_DB: OnceCell<std::sync::Mutex<PickleDb>> = OnceCell::new();

#[derive(Parser)] // requires `derive` feature
#[command(name = "photobot")]
#[command(bin_name = "photobot")]
enum Cargo {
    Import(Import),
    Test(Test),
}

#[derive(clap::Args)]
#[command(author, version, about, long_about = None)]
struct Import {
    /// Output directory for photos
    #[arg(long, short)]
    output: PathBuf,
    #[arg(long, short)]
    album_from_filename: bool,
    /// Files or directories to organize
    paths: Vec<PathBuf>,
}

#[derive(clap::Args)]
#[command(author, version, about, long_about = None)]
struct Test {
    /// Output directory for photos
    #[arg(long, short)]
    output: PathBuf,
    #[arg(long, short)]
    album_from_filename: bool,
    /// Files or directories to organize
    paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Photo {
    input_path: PathBuf,
    original_filename: Option<String>,
    output_filename: String,
    exif: Exif,
    _checksum: u32,
}

struct PhotoPath {
    input_path: PathBuf,
    input_dir: PathBuf,
}

// #[derive(Clone)]
struct State {
    output_dir: PathBuf,
    album_from_filename: bool,
}

fn main() -> Result<()> {
    if let Cargo::Import(args) = Cargo::parse() {
        PHOTOHASH_DB
            .set(std::sync::Mutex::new(load_db(&args.output)))
            .map_err(|_e| anyhow!("PhotoHashDB already initialized."))?;

        let state = State {
            output_dir: args.output,
            album_from_filename: args.album_from_filename,
        };

        if let Ok(_file) = File::open(state.output_dir.join("/photohash.db")) {}

        import_photos(&args.paths, &state);
    }

    Ok(())
}

fn _lift_state<T, S>(state: S) -> impl FnOnce(T) -> (T, S) {
    move |i: T| (i, state)
}

fn import_photos(paths: &[PathBuf], state: &State) -> Vec<Photo> {
    paths
        .iter()
        .flat_map(find_all_photos)
        .filter_map(|p| {
            import_single_photo(&p, state)
                .inspect_err(|e| eprintln!("{e}"))
                .ok()
        })
        .collect::<Vec<_>>()
}

fn import_single_photo(path: &PhotoPath, state: &State) -> Result<Photo> {
    get_photo(path, state).and_then(|photo| copy_photo(photo, state))
}

fn find_all_photos<P: AsRef<Path> + Copy>(input_dir: P) -> Vec<PhotoPath> {
    WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|p| p.ok())
        .map(|d| d.into_path())
        .filter(|p| GLOB_MATCHER.is_match(p))
        .map(|p| {
            println!(
                "\x1b[36mVerbose (find_all_photos):\x1b[0m Found {}",
                p.display()
            );
            p
        })
        .map(|p| PhotoPath {
            input_path: p,
            input_dir: input_dir.as_ref().to_path_buf(),
        })
        .collect::<Vec<_>>()
}

fn get_photo(path: &PhotoPath, state: &State) -> Result<Photo> {
    let file = File::open(&path.input_path)?;
    let mut file = BufReader::new(file);

    let checksum = adler32(&mut file)?;

    let mut exif = get_exif(&path.input_path)?;

    let extension = path
        .input_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    if state.album_from_filename
        && path.input_path.ancestors().count() - 1 > path.input_dir.ancestors().count()
    {
        exif.album = path
            .input_path
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string());
    };

    let filename =
        generate_filename(&exif).map(|file_prefix| format!("{}.{}", file_prefix, extension))?;

    Ok(Photo {
        input_path: path.input_path.to_path_buf(),
        // output_path: state.output_dir.join(filename)
        original_filename: path
            .input_path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned()),
        output_filename: filename,
        exif,
        _checksum: checksum,
    })
}

fn generate_camera(exif: &Exif) -> Option<String> {
    match (&exif.make, &exif.model) {
        (Some(make), Some(model)) => Some(format!("{} {}", make, model)),
        _ => None,
    }
}

fn generate_filename(exif: &Exif) -> Result<String> {
    let date = exif
        .date_time_original
        .or(exif.create_date)
        .ok_or_else(|| anyhow!("EXIF data is missing DateTime"))?;

    let mut s = match &exif.album {
        Some(i) => format!("albums/{}", i),
        None => format!("timeline/{}", date.format("%Y-%m-%b")),
    };

    match generate_camera(exif) {
        Some(camera) => s.push_str(format!("/{}", camera).as_str()),
        None => s.push_str("/unknown camera"),
    }

    s.push_str(format!("/{}", date.format("%Y-%m-%d_%H-%M-%S")).as_str());

    Ok(s)
}

fn copy_photo(photo: Photo, state: &State) -> Result<Photo> {
    let output_filename = format!(
        "{}/{}",
        state.output_dir.to_string_lossy(),
        photo.output_filename
    );
    let output_path = Path::new(&output_filename);

    if let Ok(_file) = File::open(output_path) {
        println!(
            "\x1b[36mVerbose (copy_photos\x1b[35;1m {}\x1b[36m):\x1b[0m Canceling copy: output file already exists",
            &photo.input_path.to_string_lossy()
        );
    } else {
        if let Some(output_dirs) = output_path.parent() {
            println!(
                "\x1b[36mVerbose (copy_photos\x1b[35;1m {}\x1b[36m):\x1b[0m Creating output directory: \x1b[35;1m{}\x1b[0m",
                &photo.input_path.to_string_lossy(),
                output_dirs.to_string_lossy()
            );
            std::fs::create_dir_all(output_dirs)?
        }

        println!(
            "\x1b[36mVerbose (copy_photos\x1b[35;1m {}\x1b[36m):\x1b[0m Copying photo to: \x1b[35;1m{}\x1b[0m",
            &photo.input_path.to_string_lossy(),
            output_path.to_string_lossy()
        );
        copy(photo.input_path.as_path(), output_path)?;
        write_exif(output_path, &photo)?;
        write_photohash(&photo)?;
    }

    Ok(photo)
}

fn write_photohash(photo: &Photo) -> Result<()> {
    let db_mutex = PHOTOHASH_DB
        .get()
        .ok_or_else(|| anyhow!("Unable to open photohash db"))?;

    let mut db = db_mutex.lock().map_err(|e| anyhow!(e.to_string()))?;

    db.set(photo._checksum.to_string().as_str(), &photo.output_filename)?;
    Ok(())
}
