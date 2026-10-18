// Interpreta as linhas que o yt-dlp imprime (marcadores definidos em plugins/ytdlp/src/args.rs).

export interface Progresso {
  fracao: number | null;
  velocidade: number | null;
  eta: number | null;
}

export type Leitura =
  | { tipo: "progresso"; valor: Progresso }
  | { tipo: "item"; titulo: string | null; indice: number | null; total: number | null }
  | { tipo: "convertendo" }
  | { tipo: "arquivo" }
  | { tipo: "erro"; mensagem: string }
  | null;

export function lerLinha(linha: string): Leitura {
  const l = linha.trim();
  try {
    if (l.startsWith("@@P")) {
      const p = JSON.parse(l.slice(3));
      const total = p.total_bytes ?? p.total_bytes_estimate;
      let fracao: number | null = null;
      if (total && p.downloaded_bytes != null) fracao = p.downloaded_bytes / total;
      else if (p.fragment_count && p.fragment_index != null) fracao = p.fragment_index / p.fragment_count;
      if (p.status === "finished") fracao = 1;
      return {
        tipo: "progresso",
        valor: { fracao: fracao == null ? null : Math.min(1, fracao), velocidade: p.speed ?? null, eta: p.eta ?? null },
      };
    }
    if (l.startsWith("@@I")) {
      const i = JSON.parse(l.slice(3));
      return { tipo: "item", titulo: i.title ?? null, indice: i.playlist_index ?? null, total: i.n_entries ?? null };
    }
  } catch {
    return null;
  }
  if (l.startsWith("@@C")) return { tipo: "convertendo" };
  if (l.startsWith("@@F")) return { tipo: "arquivo" };
  if (l.startsWith("ERROR:")) return { tipo: "erro", mensagem: l.slice(6).trim() };
  return null;
}

export function formatarBytes(b: number | null): string {
  if (!b) return "";
  const u = ["B/s", "KB/s", "MB/s", "GB/s"];
  let i = 0;
  while (b >= 1024 && i < u.length - 1) {
    b /= 1024;
    i++;
  }
  return `${b.toFixed(b < 10 && i > 0 ? 1 : 0)} ${u[i]}`;
}

export function formatarDuracao(s: number | null): string {
  if (s == null || !isFinite(s)) return "";
  s = Math.round(s);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const seg = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${seg}` : `${m}:${seg}`;
}
