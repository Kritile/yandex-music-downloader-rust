use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Artist {
    pub id: Option<u64>,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackPosition {
    pub volume: Option<u32>,
    pub index: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Album {
    pub id: Option<u64>,
    pub title: String,
    pub version: Option<String>,
    pub artists: Vec<Artist>,
    pub track_position: Option<TrackPosition>,
    pub track_count: Option<usize>,
    pub year: Option<u32>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    pub available: Option<bool>,
    pub meta_type: Option<String>,
    #[serde(skip)]
    pub volume_sizes: Vec<usize>,
    pub volumes: Vec<Vec<Track>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricsInfo {
    pub has_available_sync_lyrics: bool,
    pub has_available_text_lyrics: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Track {
    #[serde(deserialize_with = "string_id")]
    pub id: String,
    pub title: String,
    pub version: Option<String>,
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub available: Option<bool>,
    pub cover_uri: Option<String>,
    pub lyrics_info: Option<LyricsInfo>,
}

fn string_id<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => String::new(),
        _ => return Err(serde::de::Error::custom("invalid track id")),
    })
}

pub fn full_title(title: &str, version: Option<&str>) -> String {
    match version.filter(|s| !s.is_empty()) {
        Some(version) => format!("{title} ({version})"),
        None => title.to_owned(),
    }
}
