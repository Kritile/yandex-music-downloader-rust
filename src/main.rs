use std::{collections::HashMap, fs, path::Path, thread, time::Duration};

use anyhow::{bail, Result};
use clap::Parser;
use yandex_music_downloader::{
    api::MusicClient,
    cli::{Cli, LyricsFormat},
    download::{decrypt_data, file_extension, write_atomic},
    paths::prepare_base_path,
    tags::{cover_extension, write_tags},
};

fn run() -> Result<()> {
    let mut args = Cli::parse();
    let source = args.source()?;
    let cover_size = args.cover_size()?;
    if args.add_lyrics {
        eprintln!("Аргумент --add-lyrics устарел. Используйте --lyrics-format text");
        args.lyrics_format = LyricsFormat::Text;
    }
    let client = MusicClient::new(
        args.token,
        args.timeout,
        args.tries,
        args.retry_delay,
        args.debug,
    )?;
    let tracks = client.collect_tracks(&source, args.only_music, args.stick_to_artist)?;
    let total = tracks.len();
    let mut covers = HashMap::<(u64, String), Vec<u8>>::new();
    for (index, track) in tracks.iter().enumerate() {
        let progress = format!("[{}/{}]", index + 1, total);
        if track.available != Some(true) {
            println!(
                "{progress} Трек {} - {} недоступен",
                track
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                track.title
            );
            continue;
        }
        let base = args.dir.join(prepare_base_path(
            &args.path_pattern,
            track,
            args.unsafe_path,
        ));
        if args.skip_existing
            && [".mp3", ".flac", ".m4a"]
                .iter()
                .any(|ext| Path::new(&format!("{}{}", base.display(), ext)).is_file())
        {
            continue;
        }
        let info = client.download_info(&track.id, args.quality)?;
        let container = info.container()?;
        let target =
            std::path::PathBuf::from(format!("{}{}", base.display(), file_extension(container)));
        let parent = target
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Путь без каталога"))?;
        fs::create_dir_all(parent)?;
        println!(
            "{progress} [{} {}kbps] Загружается {}",
            info.codec,
            info.bitrate,
            target.display()
        );

        let mut text_lyrics = None;
        if let Some(lyrics_info) = &track.lyrics_info {
            if args.lyrics_format == LyricsFormat::Lrc && lyrics_info.has_available_sync_lyrics {
                let lrc_path = yandex_music_downloader::download::lyrics_sidecar_path(&target);
                if !lrc_path.exists() {
                    let lyrics = client.lyrics(&track.id, "LRC")?;
                    write_atomic(&lrc_path, lyrics.as_bytes(), |_| Ok(()))?;
                }
            } else if args.lyrics_format != LyricsFormat::None
                && lyrics_info.has_available_text_lyrics
            {
                text_lyrics = Some(client.lyrics(&track.id, "TEXT")?);
            }
        }

        let mut cover = None;
        if let Some(uri) = &track.cover_uri {
            let album_id = track.albums.first().and_then(|a| a.id).unwrap_or(0);
            let cache_key = (album_id, cover_size.clone());
            if !covers.contains_key(&cache_key) {
                covers.insert(cache_key.clone(), client.cover(uri, &cover_size)?);
            }
            let bytes = covers.get(&cache_key).expect("cover cache entry");
            if args.embed_cover {
                cover = Some(bytes.as_slice());
            } else {
                let ext = cover_extension(bytes)?;
                let path = parent.join(format!("cover{ext}"));
                if !path.exists() {
                    write_atomic(&path, bytes, |_| Ok(()))?;
                }
            }
        }

        let url = info
            .urls
            .first()
            .ok_or_else(|| anyhow::anyhow!("Нет ссылки на аудиофайл"))?;
        let mut data = client.bytes(url)?;
        if let Some(key) = &info.key {
            data = decrypt_data(&data, key)?;
        }
        if data.is_empty() {
            bail!("Получен пустой аудиофайл для трека {}", track.id);
        }
        write_atomic(&target, &data, |tmp| {
            write_tags(
                tmp,
                track,
                container,
                text_lyrics.as_deref(),
                cover,
                args.compatibility_level,
            )
        })?;
        if args.delay > 0 {
            thread::sleep(Duration::from_secs(args.delay));
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Ошибка: {error:#}");
        std::process::exit(1);
    }
}
