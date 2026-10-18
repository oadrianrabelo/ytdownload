use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// MP4 (H.264 + AAC): o formato de vídeo mais compatível.
    Video,
    /// MP3.
    Audio,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub id: String,
    pub url: String,
    pub kind: Kind,
    /// Altura máxima do vídeo (360, 480, 720 ou 1080).
    #[serde(default = "qualidade_padrao")]
    pub quality: u32,
    /// Baixar a playlist inteira (quando o link tem uma).
    #[serde(default)]
    pub playlist: bool,
    /// Trecho opcional: início e fim no formato 1:23 ou 1:02:03.
    pub start: Option<String>,
    pub end: Option<String>,
}

fn qualidade_padrao() -> u32 {
    720
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFile {
    pub name: String,
    /// Caminho no computador, ou endereço content:// no Android.
    pub path: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub files: Vec<SavedFile>,
    /// Preenchido quando parte da playlist falhou mas algo foi salvo.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub title: String,
    pub uploader: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub is_playlist: bool,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub ready: bool,
    pub version: Option<String>,
    pub output_dir: Option<String>,
    pub platform: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedUrl {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlArgs {
    pub url: String,
    #[serde(default)]
    pub playlist: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathArgs {
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdArgs {
    pub id: String,
}

/// Mensagens enviadas à interface durante a preparação e os downloads.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// Linha de saída do yt-dlp (a interface interpreta o progresso).
    Line { line: String },
    /// Etapa da preparação do motor.
    Stage { message: String, progress: Option<f64> },
}
