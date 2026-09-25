use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{ArgGroup, Parser, ValueEnum};

pub const DEFAULT_PATTERN: &str = "#album-artist/#album/#number - #title";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LyricsFormat {
    None,
    Text,
    Lrc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Artist(String),
    Album(String),
    Track(String),
    Playlist(String),
}

#[derive(Parser, Debug)]
#[command(
    name = "yandex-music-downloader",
    about = "Загрузчик музыки с сервиса Яндекс.Музыка"
)]
#[command(group(ArgGroup::new("source").required(true).multiple(false).args(["artist_id", "album_id", "track_id", "playlist_id", "url"])))]
pub struct Cli {
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=2), help = "Качество: 0 — AAC 64, 1 — AAC 192, 2 — FLAC")]
    pub quality: u8,
    #[arg(long)]
    pub skip_existing: bool,
    #[arg(long, value_enum, default_value_t = LyricsFormat::None)]
    pub lyrics_format: LyricsFormat,
    #[arg(long, hide = true)]
    pub add_lyrics: bool,
    #[arg(long)]
    pub embed_cover: bool,
    #[arg(long, default_value = "400")]
    pub cover_resolution: String,
    #[arg(long, default_value_t = 0)]
    pub delay: u64,
    #[arg(long)]
    pub stick_to_artist: bool,
    #[arg(long)]
    pub only_music: bool,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=1))]
    pub compatibility_level: u8,
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: u64,
    #[arg(long, default_value_t = 20)]
    pub tries: u32,
    #[arg(long, default_value_t = 5)]
    pub retry_delay: u64,
    #[arg(long, hide = true)]
    pub debug: bool,
    #[arg(long)]
    pub artist_id: Option<String>,
    #[arg(long)]
    pub album_id: Option<String>,
    #[arg(long)]
    pub track_id: Option<String>,
    #[arg(long)]
    pub playlist_id: Option<String>,
    #[arg(short = 'u', long)]
    pub url: Option<String>,
    #[arg(long)]
    pub unsafe_path: bool,
    #[arg(long, default_value = ".")]
    pub dir: PathBuf,
    #[arg(long, default_value = DEFAULT_PATTERN)]
    pub path_pattern: String,
    #[arg(long, env = "YANDEX_MUSIC_TOKEN", hide_env_values = true)]
    pub token: String,
}

impl Cli {
    pub fn source(&self) -> Result<Source> {
        if let Some(id) = &self.artist_id {
            return Ok(Source::Artist(id.clone()));
        }
        if let Some(id) = &self.album_id {
            return Ok(Source::Album(id.clone()));
        }
        if let Some(id) = &self.track_id {
            return Ok(Source::Track(id.clone()));
        }
        if let Some(id) = &self.playlist_id {
            return Ok(Source::Playlist(id.clone()));
        }
        parse_source_url(self.url.as_deref().unwrap_or_default())
    }

    pub fn cover_size(&self) -> Result<String> {
        if self.cover_resolution == "original" {
            return Ok("orig".into());
        }
        let n: u32 = self.cover_resolution.parse()?;
        if n < 100 {
            bail!("Разрешение обложки должно быть не меньше 100");
        }
        Ok(format!("{n}x{n}"))
    }
}

pub fn parse_source_url(value: &str) -> Result<Source> {
    let url = url::Url::parse(value)?;
    let parts: Vec<_> = url
        .path_segments()
        .map(|s| s.filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    match parts.as_slice() {
        ["artist", id] if numeric(id) => Ok(Source::Artist((*id).into())),
        ["album", id] if numeric(id) => Ok(Source::Album((*id).into())),
        ["track", id] if numeric(id) => Ok(Source::Track((*id).into())),
        ["album", _, "track", id] if numeric(id) => Ok(Source::Track((*id).into())),
        ["users", owner, "playlists", kind] => Ok(Source::Playlist(format!("{owner}/{kind}"))),
        ["playlists", owner, kind] => Ok(Source::Playlist(format!("{owner}/{kind}"))),
        ["playlists", uuid] => Ok(Source::Playlist((*uuid).into())),
        _ => bail!("Неверный формат URL"),
    }
}
