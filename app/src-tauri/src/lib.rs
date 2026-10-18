#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    builder
        .plugin(tauri_plugin_ytdlp::init())
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o YTDownload");
}
