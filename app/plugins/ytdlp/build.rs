const COMMANDS: &[&str] = &[
    "status",
    "prepare",
    "update",
    "inspect",
    "download",
    "cancel",
    "open_file",
    "reveal_file",
    "share_file",
    "open_output_dir",
    "choose_output_dir",
    "take_shared_url",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
