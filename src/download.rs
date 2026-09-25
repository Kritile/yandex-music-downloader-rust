use std::{fs, path::Path};

use aes::Aes128;
use anyhow::{anyhow, bail, Context, Result};
use ctr::cipher::{KeyIvInit, StreamCipher};
use serde::Deserialize;
use sha2::{Digest, Sha256};

type Aes128Ctr = ctr::Ctr128BE<Aes128>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Mp3,
    Mp4,
    Flac,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    pub quality: String,
    pub codec: String,
    pub urls: Vec<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub bitrate: u32,
}

impl DownloadInfo {
    pub fn container(&self) -> Result<Container> {
        match self.codec.as_str() {
            "mp3" => Ok(Container::Mp3),
            "flac" => Ok(Container::Flac),
            "flac-mp4" | "aac" | "he-aac" | "aac-mp4" | "he-aac-mp4" => Ok(Container::Mp4),
            other => bail!("Неподдерживаемый кодек: {other}"),
        }
    }
}

pub fn file_extension(container: Container) -> &'static str {
    match container {
        Container::Mp3 => ".mp3",
        Container::Mp4 => ".m4a",
        Container::Flac => ".flac",
    }
}

pub fn lyrics_sidecar_path(audio_path: &Path) -> std::path::PathBuf {
    audio_path.with_extension("lrc")
}

pub fn decrypt_data(data: &[u8], key_hex: &str) -> Result<Vec<u8>> {
    let key: Vec<u8> = key_hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair)?, 16).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;
    if key_hex.len() != 32 {
        bail!("Ключ AES должен содержать 32 шестнадцатеричных символа");
    }
    let mut cipher =
        Aes128Ctr::new_from_slices(&key, &[0; 16]).map_err(|_| anyhow!("Неверный ключ AES"))?;
    let mut result = data.to_vec();
    cipher.apply_keystream(&mut result);
    Ok(result)
}

pub fn write_atomic(
    path: &Path,
    bytes: &[u8],
    hook: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let name = path
        .file_name()
        .context("Отсутствует имя файла")?
        .to_string_lossy();
    let digest = Sha256::digest(name.as_bytes());
    let tmp = path.with_file_name(format!(".yandex-music-downloader.{digest:x}.tmp"));
    fs::write(&tmp, bytes).with_context(|| format!("Не удалось записать {}", tmp.display()))?;
    let result = hook(&tmp).and_then(|()| fs::rename(&tmp, path).map_err(anyhow::Error::from));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
