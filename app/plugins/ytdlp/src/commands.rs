use tauri::ipc::Channel;
use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::Result;
use crate::YtdlpExt;

#[command]
pub(crate) async fn status<R: Runtime>(app: AppHandle<R>) -> Result<EngineStatus> {
    app.ytdlp().status().await
}

#[command]
pub(crate) async fn prepare<R: Runtime>(app: AppHandle<R>, on_event: Channel<Event>) -> Result<EngineStatus> {
    app.ytdlp().prepare(on_event).await
}

#[command]
pub(crate) async fn update<R: Runtime>(app: AppHandle<R>) -> Result<String> {
    app.ytdlp().update().await
}

#[command]
pub(crate) async fn inspect<R: Runtime>(app: AppHandle<R>, args: UrlArgs) -> Result<MediaInfo> {
    app.ytdlp().inspect(args.url, args.playlist).await
}

#[command]
pub(crate) async fn download<R: Runtime>(
    app: AppHandle<R>,
    request: DownloadRequest,
    on_event: Channel<Event>,
) -> Result<DownloadResult> {
    app.ytdlp().download(request, on_event).await
}

#[command]
pub(crate) async fn cancel<R: Runtime>(app: AppHandle<R>, args: IdArgs) -> Result<()> {
    app.ytdlp().cancel(args.id)
}

#[command]
pub(crate) async fn open_file<R: Runtime>(app: AppHandle<R>, args: PathArgs) -> Result<()> {
    app.ytdlp().open_file(args.path)
}

#[command]
pub(crate) async fn reveal_file<R: Runtime>(app: AppHandle<R>, args: PathArgs) -> Result<()> {
    app.ytdlp().reveal_file(args.path)
}

#[command]
pub(crate) async fn share_file<R: Runtime>(app: AppHandle<R>, args: PathArgs) -> Result<()> {
    app.ytdlp().share_file(args.path)
}

#[command]
pub(crate) async fn open_output_dir<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.ytdlp().open_output_dir()
}

#[command]
pub(crate) async fn choose_output_dir<R: Runtime>(app: AppHandle<R>) -> Result<EngineStatus> {
    app.ytdlp().choose_output_dir().await
}

#[command]
pub(crate) async fn take_shared_url<R: Runtime>(app: AppHandle<R>) -> Result<SharedUrl> {
    app.ytdlp().take_shared_url()
}
