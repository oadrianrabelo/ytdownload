// Evita a janela de terminal extra no Windows. NÃO REMOVER.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ytdownload_lib::run();
}
