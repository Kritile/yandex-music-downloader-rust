use std::path::PathBuf;

use crate::model::{full_title, Track};

fn clean(value: &str, unsafe_path: bool) -> String {
    let value = if value.is_empty() { "None" } else { value };
    let value = value.replace(['/', '\\'], "_");
    if unsafe_path {
        return value;
    }
    value
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | '\'' | '(' | ')' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn trim_component(part: &str, max_bytes: usize) -> String {
    let mut end = part.len().min(max_bytes);
    while !part.is_char_boundary(end) {
        end -= 1;
    }
    part[..end].to_owned()
}

pub fn prepare_base_path(pattern: &str, track: &Track, unsafe_path: bool) -> PathBuf {
    let album = track.albums.first();
    let position = album.and_then(|a| a.track_position.as_ref());
    let volume = position.and_then(|p| p.volume);
    let index = position.and_then(|p| p.index);
    let volume_count = album.map_or(0, |a| a.volume_sizes.len());
    let multi_disc = volume_count > 1 || volume.unwrap_or(0) > 1;
    let explicit_disc = pattern.contains("#disc-number");
    let max_tracks = album.and_then(|a| a.volume_sizes.iter().max().copied().or(a.track_count));
    let width = max_tracks.map_or(2, |n| n.to_string().len());
    let number = index.map(|n| {
        if multi_disc && !explicit_disc {
            format!("{}.{}", volume.unwrap_or(1), n)
        } else {
            n.to_string()
        }
    });
    let padded = index.map(|n| {
        if multi_disc && !explicit_disc {
            format!("{}.{n:0width$}", volume.unwrap_or(1))
        } else {
            format!("{n:0width$}")
        }
    });
    let album_artist = album.and_then(|a| a.artists.first());
    let track_artist = track.artists.first();
    let values = [
        ("#number-padded", padded),
        ("#number", number),
        ("#disc-number-padded", volume.map(|n| format!("{n:02}"))),
        ("#disc-number", volume.map(|n| n.to_string())),
        ("#album-artist", album_artist.map(|a| a.name.clone())),
        ("#track-artist", track_artist.map(|a| a.name.clone())),
        (
            "#artist-id",
            track_artist.and_then(|a| a.id).map(|n| n.to_string()),
        ),
        ("#album-id", album.and_then(|a| a.id).map(|n| n.to_string())),
        ("#track-id", Some(track.id.clone())),
        (
            "#title",
            Some(full_title(&track.title, track.version.as_deref())),
        ),
        (
            "#album",
            album.map(|a| full_title(&a.title, a.version.as_deref())),
        ),
        ("#year", album.and_then(|a| a.year).map(|n| n.to_string())),
    ];
    let mut rendered = pattern.to_owned();
    for (key, value) in values {
        rendered = rendered.replace(key, &clean(value.as_deref().unwrap_or("None"), unsafe_path));
    }
    let mut path = if std::path::Path::new(&rendered).is_absolute() {
        PathBuf::from(std::path::MAIN_SEPARATOR.to_string())
    } else {
        PathBuf::new()
    };
    for part in rendered.split(['/', '\\']).filter(|part| !part.is_empty()) {
        path.push(trim_component(part, 250));
    }
    path
}
