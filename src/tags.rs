use std::path::Path;

use anyhow::{bail, Result};
use lofty::{
    config::WriteOptions,
    picture::{Picture, PictureType},
    tag::{ItemKey, ItemValue, Tag, TagExt, TagItem, TagType},
};

use crate::{
    download::Container,
    model::{full_title, Track},
};

pub fn cover_extension(data: &[u8]) -> Result<&'static str> {
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Ok(".jpg")
    } else if data.starts_with(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10]) {
        Ok(".png")
    } else {
        bail!("Неизвестный формат обложки")
    }
}

pub fn write_tags(
    path: &Path,
    track: &Track,
    container: Container,
    lyrics: Option<&str>,
    cover: Option<&[u8]>,
    compatibility: u8,
) -> Result<()> {
    let tag_type = match container {
        Container::Mp3 => TagType::Id3v2,
        Container::Mp4 => TagType::Mp4Ilst,
        Container::Flac => TagType::VorbisComments,
    };
    let mut tag = Tag::new(tag_type);
    let album = track.albums.first();
    tag.insert_text(
        ItemKey::TrackTitle,
        full_title(&track.title, track.version.as_deref()),
    );
    tag.insert_text(
        ItemKey::AlbumTitle,
        album
            .map(|a| full_title(&a.title, a.version.as_deref()))
            .unwrap_or_default(),
    );
    let artists: Vec<&str> = track
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .filter(|n| !n.is_empty())
        .collect();
    let album_artists: Vec<&str> = album
        .into_iter()
        .flat_map(|a| a.artists.iter())
        .map(|a| a.name.as_str())
        .filter(|n| !n.is_empty())
        .collect();
    let multi = container != Container::Mp4 || compatibility == 0;
    if multi {
        for artist in &artists {
            tag.push(TagItem::new(
                ItemKey::TrackArtist,
                ItemValue::Text((*artist).into()),
            ));
        }
        for artist in &album_artists {
            tag.push(TagItem::new(
                ItemKey::AlbumArtist,
                ItemValue::Text((*artist).into()),
            ));
        }
    } else {
        let separator = if container == Container::Mp4 {
            "; "
        } else {
            "/"
        };
        tag.insert_text(ItemKey::TrackArtist, artists.join(separator));
        tag.insert_text(ItemKey::AlbumArtist, album_artists.join(separator));
    }
    if let Some(album) = album {
        if let Some(date) = album.release_date.as_deref() {
            tag.insert_text(ItemKey::RecordingDate, date.into());
        } else if let Some(year) = album.year {
            tag.insert_text(ItemKey::RecordingDate, year.to_string());
        }
        if let Some(genre) = &album.genre {
            tag.insert_text(ItemKey::Genre, genre.clone());
        }
        if let Some(position) = &album.track_position {
            if let Some(n) = position.index {
                tag.insert_text(ItemKey::TrackNumber, n.to_string());
            }
            if let Some(n) = position.volume {
                tag.insert_text(ItemKey::DiscNumber, n.to_string());
            }
        }
    }
    if let Some(lyrics) = lyrics {
        tag.insert_text(ItemKey::UnsyncLyrics, lyrics.into());
    }
    let album_id = album
        .and_then(|a| a.id)
        .map(|n| n.to_string())
        .unwrap_or_else(|| "None".into());
    let track_url = format!(
        "https://music.yandex.ru/album/{album_id}/track/{}",
        track.id
    );
    tag.insert_text(
        if container == Container::Mp3 {
            ItemKey::AudioFileUrl
        } else {
            ItemKey::Comment
        },
        track_url,
    );
    if let Some(bytes) = cover {
        cover_extension(bytes)?;
        let mut reader = bytes;
        let mut picture = Picture::from_reader(&mut reader)?;
        picture.set_pic_type(PictureType::CoverFront);
        tag.push_picture(picture);
    }
    tag.save_to_path(path, WriteOptions::default())?;
    Ok(())
}
