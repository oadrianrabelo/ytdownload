use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod args;
mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Ytdlp;
#[cfg(mobile)]
use mobile::Ytdlp;

pub trait YtdlpExt<R: Runtime> {
    fn ytdlp(&self) -> &Ytdlp<R>;
}

impl<R: Runtime, T: Manager<R>> crate::YtdlpExt<R> for T {
    fn ytdlp(&self) -> &Ytdlp<R> {
        self.state::<Ytdlp<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("ytdlp")
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::prepare,
            commands::update,
            commands::inspect,
            commands::download,
            commands::cancel,
            commands::open_file,
            commands::reveal_file,
            commands::share_file,
            commands::open_output_dir,
            commands::choose_output_dir,
            commands::take_shared_url,
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let ytdlp = mobile::init(app, api)?;
            #[cfg(desktop)]
            let ytdlp = desktop::init(app, api)?;
            app.manage(ytdlp);
            Ok(())
        })
        .build()
}
