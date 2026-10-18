//! Motor no Android: a parte nativa (Kotlin, pasta android/) roda o yt-dlp com a
//! biblioteca youtubedl-android. Os argumentos continuam sendo montados aqui em Rust.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::args;
use crate::models::*;
use crate::{Error, Result};

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<Ytdlp<R>> {
    let handle = api.register_android_plugin("com.adrianrabelo.ytdlp", "YtdlpPlugin")?;
    Ok(Ytdlp(handle))
}

pub struct Ytdlp<R: Runtime>(PluginHandle<R>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunArgs {
    id: Option<String>,
    args: Vec<String>,
    folder: Option<String>,
    channel: Option<Channel<Event>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunResult {
    exit_code: i32,
    out: String,
    err: String,
    #[serde(default)]
    files: Vec<SavedFile>,
    #[serde(default)]
    canceled: bool,
}

#[derive(Serialize)]
struct Vazio {}

impl<R: Runtime> Ytdlp<R> {
    async fn chamar<T: DeserializeOwned>(&self, comando: &str, dados: impl Serialize) -> Result<T> {
        self.0
            .run_mobile_plugin_async(comando, dados)
            .await
            .map_err(Into::into)
    }

    pub async fn status(&self) -> Result<EngineStatus> {
        self.chamar("status", Vazio {}).await
    }

    pub async fn prepare(&self, channel: Channel<Event>) -> Result<EngineStatus> {
        #[derive(Serialize)]
        struct A {
            channel: Channel<Event>,
        }
        self.chamar("prepare", A { channel }).await
    }

    pub async fn update(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct V {
            version: String,
        }
        Ok(self.chamar::<V>("update", Vazio {}).await?.version)
    }

    pub async fn inspect(&self, url: String, playlist: bool) -> Result<MediaInfo> {
        let r: RunResult = self
            .chamar(
                "run",
                RunArgs { id: None, args: args::args_inspecionar(&url, playlist)?, folder: None, channel: None },
            )
            .await?;
        if r.exit_code != 0 && r.out.trim().is_empty() {
            return Err(Error::msg(
                args::ultimo_erro(&r.err).unwrap_or_else(|| "Não consegui abrir esse link.".into()),
            ));
        }
        args::interpretar_info(&r.out)
    }

    pub async fn download(&self, req: DownloadRequest, channel: Channel<Event>) -> Result<DownloadResult> {
        let pasta = match req.kind {
            Kind::Video => "Vídeos",
            Kind::Audio => "Áudios",
        };
        let r: RunResult = self
            .chamar(
                "run",
                RunArgs {
                    id: Some(req.id.clone()),
                    args: args::args_baixar(&req)?,
                    folder: Some(pasta.into()),
                    channel: Some(channel),
                },
            )
            .await?;
        if r.canceled {
            return Err(Error::msg("Cancelado."));
        }
        let erro = args::ultimo_erro(&r.err);
        if r.exit_code == 0 || !r.files.is_empty() {
            Ok(DownloadResult { files: r.files, error: if r.exit_code == 0 { None } else { erro } })
        } else {
            Err(Error::msg(erro.unwrap_or_else(|| "O download falhou.".into())))
        }
    }

    pub fn cancel(&self, id: String) -> Result<()> {
        self.0.run_mobile_plugin::<serde_json::Value>("cancel", IdArgs2 { id })?;
        Ok(())
    }

    pub fn open_file(&self, path: String) -> Result<()> {
        self.0.run_mobile_plugin::<serde_json::Value>("openFile", PathArgs2 { path })?;
        Ok(())
    }

    pub fn reveal_file(&self, path: String) -> Result<()> {
        self.open_file(path)
    }

    pub fn share_file(&self, path: String) -> Result<()> {
        self.0.run_mobile_plugin::<serde_json::Value>("shareFile", PathArgs2 { path })?;
        Ok(())
    }

    pub fn open_output_dir(&self) -> Result<()> {
        self.0.run_mobile_plugin::<serde_json::Value>("openOutputDir", Vazio {})?;
        Ok(())
    }

    pub async fn choose_output_dir(&self) -> Result<EngineStatus> {
        self.status().await
    }

    pub fn read_clipboard(&self) -> Result<SharedUrl> {
        Ok(self.0.run_mobile_plugin("readClipboard", Vazio {})?)
    }

    pub fn take_shared_url(&self) -> Result<SharedUrl> {
        Ok(self.0.run_mobile_plugin("takeSharedUrl", Vazio {})?)
    }
}

#[derive(Serialize)]
struct IdArgs2 {
    id: String,
}

#[derive(Serialize)]
struct PathArgs2 {
    path: String,
}
