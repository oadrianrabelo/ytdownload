// Ponte entre a interface e o motor (plugin "ytdlp", em Rust + Kotlin no Android).
import { Channel, invoke } from "@tauri-apps/api/core";

export type Kind = "video" | "audio";
export type AudioFormat = "mp3" | "m4a";

export interface EngineStatus {
  ready: boolean;
  version: string | null;
  outputDir: string | null;
  platform: string;
}

export interface Entry {
  id: string;
  title: string;
  duration: number | null;
}

export interface MediaInfo {
  title: string;
  uploader: string | null;
  thumbnail: string | null;
  duration: number | null;
  isPlaylist: boolean;
  entries: Entry[];
}

export interface SavedFile {
  name: string;
  path: string;
}

export interface DownloadResult {
  files: SavedFile[];
  error: string | null;
}

export interface DownloadRequest {
  id: string;
  url: string;
  kind: Kind;
  quality: number;
  playlist: boolean;
  audioFormat: AudioFormat;
  start?: string | null;
  end?: string | null;
}

export type EngineEvent =
  | { type: "line"; line: string }
  | { type: "stage"; message: string; progress?: number | null };

const cmd = <T>(nome: string, args?: Record<string, unknown>) => invoke<T>(`plugin:ytdlp|${nome}`, args);

function canal(onEvent: (e: EngineEvent) => void) {
  const c = new Channel<EngineEvent>();
  c.onmessage = onEvent;
  return c;
}

export const motor = {
  status: () => cmd<EngineStatus>("status"),
  prepare: (onEvent: (e: EngineEvent) => void) => cmd<EngineStatus>("prepare", { onEvent: canal(onEvent) }),
  update: () => cmd<string>("update"),
  inspect: (url: string, playlist: boolean) => cmd<MediaInfo>("inspect", { args: { url, playlist } }),
  download: (request: DownloadRequest, onEvent: (e: EngineEvent) => void) =>
    cmd<DownloadResult>("download", { request, onEvent: canal(onEvent) }),
  cancel: (id: string) => cmd<void>("cancel", { args: { id } }),
  openFile: (path: string) => cmd<void>("open_file", { args: { path } }),
  revealFile: (path: string) => cmd<void>("reveal_file", { args: { path } }),
  shareFile: (path: string) => cmd<void>("share_file", { args: { path } }),
  openOutputDir: () => cmd<void>("open_output_dir"),
  chooseOutputDir: () => cmd<EngineStatus>("choose_output_dir"),
  takeSharedUrl: () => cmd<{ url: string | null }>("take_shared_url"),
  readClipboard: () => cmd<{ url: string | null }>("read_clipboard"),
};

export function mensagemDeErro(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

const HOSTS = new Set([
  "youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com",
  "youtu.be", "www.youtu.be", "youtube-nocookie.com", "www.youtube-nocookie.com",
]);

/** Extrai um link do YouTube de um texto colado (o app do YouTube manda título + link). */
export function extrairLink(texto: string): string | null {
  for (const bruto of texto.match(/(https?:\/\/)?[^\s]+/g) ?? []) {
    try {
      const u = new URL(bruto.includes("://") ? bruto : `https://${bruto}`);
      if (HOSTS.has(u.hostname.toLowerCase())) return u.toString();
    } catch {
      /* não é link */
    }
  }
  return null;
}

/** "Playlist pura" (/playlist?list=) ou vídeo dentro de uma playlist (watch?v=..&list=). */
export function tipoDeLink(link: string): "video" | "playlist" | "video-em-playlist" {
  const u = new URL(link);
  const temLista = u.searchParams.has("list");
  if (u.pathname.replace(/\/$/, "") === "/playlist" && temLista) return "playlist";
  if (/^\/(@[^/]+|channel\/[^/]+|c\/[^/]+|user\/[^/]+)(\/(videos|shorts|streams|playlists))?$/.test(u.pathname))
    return "playlist";
  return temLista ? "video-em-playlist" : "video";
}
