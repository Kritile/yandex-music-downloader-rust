use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use reqwest::blocking::Client;
use serde_json::Value;
use sha2::Sha256;

use crate::{
    cli::Source,
    download::DownloadInfo,
    model::{Album, Track},
};

const BASE: &str = "https://api.music.yandex.net";
const SIGN_KEY: &[u8] = b"p93jhgh689SBReK6ghtw62";
const CODECS: &str = "flac,flac-mp4,mp3,aac,he-aac,aac-mp4,he-aac-mp4";

fn sign(message: &str) -> Result<String> {
    let mut hmac =
        Hmac::<Sha256>::new_from_slice(SIGN_KEY).map_err(|_| anyhow!("Неверный ключ подписи"))?;
    hmac.update(message.as_bytes());
    Ok(STANDARD.encode(hmac.finalize().into_bytes()))
}

pub fn file_info_signature(timestamp: u64, track_id: &str, quality: &str) -> Result<String> {
    let message = format!(
        "{timestamp}{track_id}{quality}{}encraw",
        CODECS.replace(',', "")
    );
    Ok(sign(&message)?.trim_end_matches('=').to_owned())
}

pub fn lyrics_signature(timestamp: u64, track_id: &str) -> Result<String> {
    let numeric_id = track_id.split(':').next().unwrap_or(track_id);
    sign(&format!("{numeric_id}{timestamp}"))
}

pub struct MusicClient {
    http: Client,
    base_url: url::Url,
    token: String,
    tries: u32,
    retry_delay: Duration,
    debug: bool,
}

impl MusicClient {
    pub fn new(
        token: String,
        timeout: u64,
        tries: u32,
        retry_delay: u64,
        debug: bool,
    ) -> Result<Self> {
        Ok(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(timeout))
                .user_agent("yandex-music-downloader/4.0.0")
                .build()?,
            base_url: url::Url::parse(BASE)?,
            token,
            tries,
            retry_delay: Duration::from_secs(retry_delay),
            debug,
        })
    }

    /// Set an alternate API endpoint, primarily for local integration tests.
    /// Plain HTTP is accepted only for loopback addresses.
    pub fn with_api_base_url(mut self, base_url: &str) -> Result<Self> {
        let parsed = url::Url::parse(base_url)?;
        let loopback = match parsed.host() {
            Some(url::Host::Ipv4(address)) => address.is_loopback(),
            Some(url::Host::Ipv6(address)) => address.is_loopback(),
            _ => false,
        };
        if (parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback))
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            bail!("Некорректный базовый URL API");
        }
        self.base_url = parsed;
        Ok(self)
    }

    fn request_bytes(
        &self,
        method: reqwest::Method,
        url: &str,
        params: &[(&str, String)],
        authenticated: bool,
    ) -> Result<Vec<u8>> {
        let mut attempt = 0;
        loop {
            if self.debug {
                eprintln!("{} {url} (попытка {})", method, attempt + 1);
            }
            let mut builder = self.http.request(method.clone(), url);
            if authenticated {
                builder = builder
                    .header("Authorization", format!("OAuth {}", self.token))
                    .header("X-Yandex-Music-Client", "YandexMusicAndroid/24023621");
            }
            let request = if method == reqwest::Method::POST {
                builder.form(params)
            } else {
                builder.query(params)
            };
            let response = request.send();
            let body = match response {
                Ok(response) if response.status().is_success() => Some(response.bytes()),
                Ok(response)
                    if response.status().is_server_error() || response.status().as_u16() == 429 =>
                {
                    if self.tries != 0 && attempt >= self.tries {
                        bail!("HTTP {}: {url}", response.status());
                    }
                    None
                }
                Ok(response) => {
                    let status = response.status();
                    let detail = response
                        .text()
                        .unwrap_or_default()
                        .replace(&self.token, "[redacted]");
                    bail!(
                        "HTTP {status}: {url}: {}",
                        detail.chars().take(500).collect::<String>()
                    );
                }
                Err(error) => {
                    if self.tries != 0 && attempt >= self.tries {
                        return Err(error.into());
                    }
                    None
                }
            };
            if let Some(body) = body {
                match body {
                    Ok(body) => return Ok(body.to_vec()),
                    Err(error) if self.tries != 0 && attempt >= self.tries => {
                        return Err(error.into())
                    }
                    Err(_) => {}
                }
            }
            attempt += 1;
            thread::sleep(self.retry_delay);
        }
    }

    fn json(
        &self,
        method: reqwest::Method,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<Value> {
        let url = self.base_url.join(path)?.to_string();
        let envelope: Value =
            serde_json::from_slice(&self.request_bytes(method, &url, params, true)?)?;
        if let Some(error) = envelope.get("error") {
            bail!("Ошибка API: {error}");
        }
        Ok(envelope.get("result").cloned().unwrap_or(envelope))
    }

    fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
        self.json(reqwest::Method::GET, path, params)
    }
    fn post(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
        self.json(reqwest::Method::POST, path, params)
    }

    pub fn bytes(&self, url: &str) -> Result<Vec<u8>> {
        let parsed = url::Url::parse(url)?;
        if parsed.scheme() != "https" {
            bail!("Ожидается HTTPS-ссылка для загрузки");
        }
        self.request_bytes(reqwest::Method::GET, url, &[], false)
    }

    pub fn tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        let mut params: Vec<(&str, String)> =
            ids.iter().map(|id| ("track-ids", id.clone())).collect();
        params.push(("with-positions", "True".into()));
        let value = self.post("/tracks", &params)?;
        Ok(serde_json::from_value(value)?)
    }

    pub fn album_tracks(&self, id: &str) -> Result<Vec<Track>> {
        let value = self.get(&format!("/albums/{id}/with-tracks"), &[])?;
        let album: Album = serde_json::from_value(value)?;
        let sizes: Vec<_> = album.volumes.iter().map(Vec::len).collect();
        let mut tracks = Vec::new();
        for volume in &album.volumes {
            for track in volume {
                let mut track = track.clone();
                if let Some(first) = track.albums.first_mut() {
                    first.volume_sizes = sizes.clone();
                } else {
                    let mut parent = album.clone();
                    parent.volumes.clear();
                    parent.volume_sizes = sizes.clone();
                    track.albums.push(parent);
                }
                tracks.push(track);
            }
        }
        Ok(tracks)
    }

    pub fn collect_tracks(
        &self,
        source: &Source,
        only_music: bool,
        stick_to_artist: bool,
    ) -> Result<Vec<Track>> {
        match source {
            Source::Track(id) => self.tracks(std::slice::from_ref(id)),
            Source::Album(id) => self.album_tracks(id),
            Source::Artist(id) => {
                let mut page = 0;
                let mut result = Vec::new();
                loop {
                    let value = self.get(
                        &format!("/artists/{id}/direct-albums"),
                        &[
                            ("sort-by", "year".into()),
                            ("page", page.to_string()),
                            ("page-size", "20".into()),
                        ],
                    )?;
                    let albums = value["albums"].as_array().context("Нет списка альбомов")?;
                    for item in albums {
                        let album: Album = serde_json::from_value(item.clone())?;
                        if album.available != Some(true) || album.id.is_none() {
                            continue;
                        }
                        if only_music && album.meta_type.as_deref() != Some("music") {
                            continue;
                        }
                        if stick_to_artist
                            && album
                                .artists
                                .first()
                                .and_then(|a| a.id)
                                .map(|n| n.to_string())
                                .as_deref()
                                != Some(id)
                        {
                            continue;
                        }
                        result.extend(self.album_tracks(&album.id.unwrap().to_string())?);
                    }
                    let pager = &value["pager"];
                    let total = pager["total"].as_u64().unwrap_or(0);
                    let per_page = pager["perPage"].as_u64().unwrap_or(20);
                    page += 1;
                    if albums.is_empty() || (page as u64) * per_page >= total {
                        break;
                    }
                }
                Ok(result)
            }
            Source::Playlist(id) => {
                let value = if let Some((owner, kind)) = id.split_once('/') {
                    self.get(&format!("/users/{owner}/playlists/{kind}"), &[])?
                } else {
                    self.get(&format!("/playlist/{id}"), &[])?
                };
                let entries = value["tracks"]
                    .as_array()
                    .context("Нет треков в плейлисте")?;
                let ids: Vec<String> = entries
                    .iter()
                    .filter_map(|entry| {
                        let id = &entry["id"];
                        let id = if id.is_string() {
                            id.as_str().map(str::to_owned)
                        } else {
                            id.as_u64().map(|n| n.to_string())
                        }?;
                        let album_id = &entry["albumId"];
                        Some(
                            match album_id
                                .as_u64()
                                .map(|n| n.to_string())
                                .or_else(|| album_id.as_str().map(str::to_owned))
                            {
                                Some(album_id) => format!("{id}:{album_id}"),
                                None => id,
                            },
                        )
                    })
                    .collect();
                let mut result = Vec::new();
                for chunk in ids.chunks(10) {
                    result.extend(self.tracks(chunk)?);
                }
                Ok(result)
            }
        }
    }

    pub fn download_info(&self, track_id: &str, quality: u8) -> Result<DownloadInfo> {
        let quality = match quality {
            0 => "lq",
            1 => "nq",
            _ => "lossless",
        };
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let sign = file_info_signature(timestamp, track_id, quality)?;
        let value = self.get(
            "/get-file-info",
            &[
                ("ts", timestamp.to_string()),
                ("trackId", track_id.into()),
                ("quality", quality.into()),
                ("codecs", CODECS.into()),
                ("transports", "encraw".into()),
                ("sign", sign),
            ],
        )?;
        Ok(serde_json::from_value(value["downloadInfo"].clone())?)
    }

    pub fn lyrics(&self, track_id: &str, format: &str) -> Result<String> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let sign = lyrics_signature(timestamp, track_id)?;
        let value = self.get(
            &format!("/tracks/{track_id}/lyrics"),
            &[
                ("format", format.into()),
                ("timeStamp", timestamp.to_string()),
                ("sign", sign),
            ],
        )?;
        let url = value["downloadUrl"]
            .as_str()
            .or_else(|| value["url"].as_str())
            .context("Нет ссылки на текст песни")?;
        String::from_utf8(self.bytes(url)?).context("Текст песни не является UTF-8")
    }

    pub fn cover(&self, uri: &str, size: &str) -> Result<Vec<u8>> {
        let url = if uri.starts_with("https://") {
            uri.replace("%%", size)
        } else {
            format!(
                "https://{}",
                uri.replace("%%", size).trim_start_matches("//")
            )
        };
        self.bytes(&url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn retries_truncated_response_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request_start = [0; 1];
                stream.read_exact(&mut request_start).unwrap();
                let response = if index == 0 {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nx"
                        .as_slice()
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                        .as_slice()
                };
                stream.write_all(response).unwrap();
            }
        });
        let client = MusicClient::new("test".into(), 2, 1, 0, false).unwrap();
        let response = client
            .request_bytes(
                reqwest::Method::GET,
                &format!("http://{address}"),
                &[],
                false,
            )
            .unwrap();
        assert_eq!(response, b"ok");
        server.join().unwrap();
    }
}
