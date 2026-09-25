pub mod api;
pub mod cli;
pub mod download;
pub mod model;
pub mod paths;
pub mod tags;

#[cfg(test)]
mod tests {
    use crate::cli::{parse_source_url, Source};
    use crate::download::{decrypt_data, file_extension, Container};
    use crate::model::{Album, Artist, Track, TrackPosition};
    use crate::paths::prepare_base_path;

    fn track() -> Track {
        Track {
            id: "42".into(),
            title: "Song".into(),
            version: None,
            artists: vec![Artist {
                id: Some(7),
                name: "Band".into(),
            }],
            albums: vec![Album {
                id: Some(8),
                title: "Album".into(),
                artists: vec![Artist {
                    id: Some(7),
                    name: "Band".into(),
                }],
                track_position: Some(TrackPosition {
                    volume: Some(2),
                    index: Some(3),
                }),
                track_count: Some(12),
                volume_sizes: vec![12, 10],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn parses_all_source_urls() {
        assert_eq!(
            parse_source_url("https://music.yandex.ru/artist/7").unwrap(),
            Source::Artist("7".into())
        );
        assert_eq!(
            parse_source_url("https://music.yandex.ru/album/8").unwrap(),
            Source::Album("8".into())
        );
        assert_eq!(
            parse_source_url("https://music.yandex.ru/album/8/track/42").unwrap(),
            Source::Track("42".into())
        );
        assert_eq!(
            parse_source_url("https://music.yandex.ru/users/me/playlists/9").unwrap(),
            Source::Playlist("me/9".into())
        );
        assert_eq!(
            parse_source_url("https://music.yandex.ru/playlists/uuid").unwrap(),
            Source::Playlist("uuid".into())
        );
        assert!(parse_source_url("https://music.yandex.ru/invalid/7").is_err());
    }

    #[test]
    fn renders_multidisc_number_without_disc_placeholder() {
        assert_eq!(
            prepare_base_path("#album-artist/#album/#number - #title", &track(), false)
                .to_string_lossy(),
            "Band/Album/2.3 - Song"
        );
        assert_eq!(
            prepare_base_path(
                "#disc-number-padded/#number-padded - #title",
                &track(),
                false
            )
            .to_string_lossy(),
            "02/03 - Song"
        );
    }

    #[test]
    fn decrypts_ctr_stream_and_maps_containers() {
        let key = "00000000000000000000000000000000";
        let plaintext = b"audio bytes";
        let encrypted = decrypt_data(plaintext, key).unwrap();
        assert_ne!(encrypted, plaintext);
        assert_eq!(decrypt_data(&encrypted, key).unwrap(), plaintext);
        assert_eq!(file_extension(Container::Mp4), ".m4a");
    }

    #[test]
    fn writes_metadata_to_all_supported_containers() {
        use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
        for (name, container) in [
            ("silence.mp3", Container::Mp3),
            ("silence.flac", Container::Flac),
            ("silence.m4a", Container::Mp4),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let target = temp.path().join(name);
            std::fs::copy(format!("tests/fixtures/{name}"), &target).unwrap();
            crate::tags::write_tags(&target, &track(), container, Some("lyrics"), None, 1).unwrap();
            let tagged = Probe::open(&target).unwrap().read().unwrap();
            let tag = tagged.primary_tag().unwrap();
            assert_eq!(tag.get_string(ItemKey::TrackTitle), Some("Song"), "{name}");
            assert_eq!(tag.get_string(ItemKey::AlbumTitle), Some("Album"), "{name}");
            assert_eq!(tag.get_string(ItemKey::TrackNumber), Some("3"), "{name}");
        }
    }

    #[test]
    fn tags_audio_before_atomic_rename() {
        use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("song.m4a");
        let bytes = include_bytes!("../tests/fixtures/silence.m4a");
        crate::download::write_atomic(&target, bytes, |path| {
            crate::tags::write_tags(path, &track(), Container::Mp4, None, None, 1)
        })
        .unwrap();
        let tagged = Probe::open(&target).unwrap().read().unwrap();
        assert_eq!(
            tagged
                .primary_tag()
                .unwrap()
                .get_string(ItemKey::TrackTitle),
            Some("Song")
        );
    }

    #[test]
    fn embeds_cover_and_lyrics_in_each_container() {
        use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
        for (name, container) in [
            ("silence.mp3", Container::Mp3),
            ("silence.flac", Container::Flac),
            ("silence.m4a", Container::Mp4),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let target = temp.path().join(name);
            std::fs::copy(format!("tests/fixtures/{name}"), &target).unwrap();
            crate::tags::write_tags(
                &target,
                &track(),
                container,
                Some("sample lyrics"),
                Some(include_bytes!("../tests/fixtures/cover.png")),
                1,
            )
            .unwrap();
            let tagged = Probe::open(&target).unwrap().read().unwrap();
            let tag = tagged.primary_tag().unwrap();
            let lyrics_key = if container == Container::Mp4 {
                ItemKey::Lyrics
            } else {
                ItemKey::UnsyncLyrics
            };
            assert_eq!(tag.get_string(lyrics_key), Some("sample lyrics"), "{name}");
            assert_eq!(tag.pictures().len(), 1, "{name}");
        }
    }

    #[test]
    fn mp4_compatibility_level_joins_multiple_artists() {
        use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("song.m4a");
        std::fs::copy("tests/fixtures/silence.m4a", &target).unwrap();
        let mut track = track();
        track.artists.push(Artist {
            id: Some(9),
            name: "Guest".into(),
        });
        crate::tags::write_tags(&target, &track, Container::Mp4, None, None, 1).unwrap();
        let tagged = Probe::open(&target).unwrap().read().unwrap();
        assert_eq!(
            tagged
                .primary_tag()
                .unwrap()
                .get_string(ItemKey::TrackArtist),
            Some("Band; Guest")
        );
    }

    #[test]
    fn signs_file_and_lyrics_requests_like_python_client() {
        assert_eq!(
            crate::api::file_info_signature(1234567890, "42", "nq").unwrap(),
            "VEDNeK4EO+k5ZpXfEfmLmQwf/Pb97B9Iqu/vJxAvFrE"
        );
        assert_eq!(
            crate::api::lyrics_signature(1234567890, "42:8").unwrap(),
            "MlAHXfhg4a2sgRIRnrSH3extQYvxztsuWL7iJ1WIV4k="
        );
    }

    #[test]
    fn lyrics_sidecar_keeps_dots_in_track_title() {
        let path = std::path::Path::new("Album/1.1 - Song.m4a");
        assert_eq!(
            crate::download::lyrics_sidecar_path(path),
            std::path::PathBuf::from("Album/1.1 - Song.lrc")
        );
    }

    #[test]
    fn absolute_path_pattern_stays_absolute() {
        assert_eq!(
            prepare_base_path("/tmp/music/#title", &track(), false),
            std::path::PathBuf::from("/tmp/music/Song")
        );
    }

    #[test]
    fn mp3_preserves_separate_artist_values() {
        use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("song.mp3");
        std::fs::copy("tests/fixtures/silence.mp3", &target).unwrap();
        let mut track = track();
        track.artists.push(Artist {
            id: Some(9),
            name: "Guest".into(),
        });
        crate::tags::write_tags(&target, &track, Container::Mp3, None, None, 1).unwrap();
        let tagged = Probe::open(&target).unwrap().read().unwrap();
        let artists: Vec<_> = tagged
            .primary_tag()
            .unwrap()
            .get_strings(ItemKey::TrackArtist)
            .collect();
        assert_eq!(artists, vec!["Band", "Guest"]);
    }

    #[test]
    fn mp4_level_zero_preserves_separate_artist_values() {
        use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("song.m4a");
        std::fs::copy("tests/fixtures/silence.m4a", &target).unwrap();
        let mut track = track();
        track.artists.push(Artist {
            id: Some(9),
            name: "Guest".into(),
        });
        crate::tags::write_tags(&target, &track, Container::Mp4, None, None, 0).unwrap();
        let tagged = Probe::open(&target).unwrap().read().unwrap();
        let artists: Vec<_> = tagged
            .primary_tag()
            .unwrap()
            .get_strings(ItemKey::TrackArtist)
            .collect();
        assert_eq!(artists, vec!["Band", "Guest"]);
    }
}
