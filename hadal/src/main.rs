//! Hadal - An audiophile-grade TUI music player for Linux.
//!
//! Named after the hadal zone, the deepest oceanic trenches —
//! where only the purest signals reach.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod config;

use config::Config;

/// Hadal - Audiophile-grade TUI music player for Linux
#[derive(Parser, Debug)]
#[command(name = "hadal")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to a file or directory to play
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Scan library on startup
    #[arg(long)]
    scan: bool,

    /// List audio devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Print detailed version information
    #[arg(long)]
    version_info: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle --version-info
    if args.version_info {
        print_version_info();
        return Ok(());
    }

    // Load paths first (needed by both logging and config)
    let paths = hadal_common::Paths::new().context("Failed to initialize paths")?;

    // Initialize logging
    init_logging(args.debug, &paths.log_file)?;

    // Handle --list-devices
    if args.list_devices {
        list_audio_devices()?;
        return Ok(());
    }
    let config_path = args.config.as_ref().unwrap_or(&paths.config_file);
    let config = Config::load_or_create(config_path).context("Failed to load configuration")?;

    tracing::info!("Starting Hadal v{}", env!("CARGO_PKG_VERSION"));
    tracing::debug!("Config: {:?}", config);
    tracing::debug!("Paths: {:?}", paths);

    // Run the application
    run(config, paths)
}

fn init_logging(debug: bool, log_path: &std::path::Path) -> Result<()> {
    let filter = if debug {
        EnvFilter::new("hadal=debug,hadal_audio=debug,hadal_library=debug")
    } else {
        EnvFilter::new("hadal=info")
    };

    // Write logs to file instead of stderr so they don't corrupt the TUI
    let log_file = std::fs::File::create(log_path)
        .context("Failed to create log file")?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_file(debug)
        .with_line_number(debug)
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    Ok(())
}

fn print_version_info() {
    println!("Hadal v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Build info:");
    println!("  Target:    {}-{}", std::env::consts::ARCH, std::env::consts::OS);
    println!("  Profile:   {}", if cfg!(debug_assertions) { "debug" } else { "release" });
    println!();
    println!("Supported formats:");
    println!("  Audio: FLAC, MP3, AAC/M4A, Ogg Vorbis, Opus, WAV, AIFF, ALAC");
    println!();
    println!("Features:");
    println!("  Audio output:   PipeWire");
    println!("  Database:       SQLite with FTS5");
}

fn list_audio_devices() -> Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();

    println!("Audio output devices:");
    println!();

    let default_device = host.default_output_device();
    let default_name = default_device
        .as_ref()
        .and_then(|d| d.name().ok());

    match host.output_devices() {
        Ok(devices) => {
            for device in devices {
                let name = device.name().unwrap_or_else(|_| "unknown".into());
                let is_default = default_name.as_deref() == Some(&name);
                let marker = if is_default { " (default)" } else { "" };

                print!("  {}{}", name, marker);

                if let Ok(config) = device.default_output_config() {
                    print!(
                        " — {} ch, {}Hz, {:?}",
                        config.channels(),
                        config.sample_rate().0,
                        config.sample_format(),
                    );
                }
                println!();
            }
        }
        Err(e) => {
            println!("  Error enumerating devices: {}", e);
        }
    }

    Ok(())
}

fn run(config: Config, paths: hadal_common::Paths) -> Result<()> {
    // Open library database
    let db = hadal_library::Database::open(&paths.database)
        .context("Failed to open library database")?;

    // Always scan library on launch (fast — skips unchanged files)
    scan_library(&db, &config, &paths.artwork_cache)?;

    tracing::info!("Database opened, launching TUI");

    // Build playback settings from config
    let settings = hadal_common::PlaybackSettings {
        resampler_quality: config.playback.resampler_quality.clone(),
        buffer_size: config.playback.buffer_size,
        gapless: config.playback.gapless,
        default_volume: config.playback.default_volume,
    };

    // Run the TUI
    hadal_tui::run(db, paths, settings).context("TUI error")?;

    Ok(())
}

fn scan_library(db: &hadal_library::Database, config: &Config, artwork_cache: &std::path::Path) -> Result<()> {
    let folders: Vec<PathBuf> = config
        .library
        .folders
        .iter()
        .filter_map(|f| {
            if let Some(rest) = f.strip_prefix("~/") {
                match dirs::home_dir() {
                    Some(home) => Some(home.join(rest)),
                    None => {
                        tracing::warn!("Cannot resolve home directory for path: {}", f);
                        None
                    }
                }
            } else {
                Some(PathBuf::from(f))
            }
        })
        .collect();

    let scanner = hadal_library::Scanner::new(folders);
    let files = scanner.scan_sync().context("Scan failed")?;

    let indexer = hadal_library::Indexer::with_artwork(db.clone(), artwork_cache.to_path_buf());
    let progress = indexer.index_files(&files, |_| {}).context("Indexing failed")?;

    tracing::info!(
        "Library scan: {} scanned, {} added, {} updated, {} errors",
        progress.scanned_files, progress.added, progress.updated, progress.errors
    );

    Ok(())
}
