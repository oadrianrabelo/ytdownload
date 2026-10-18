//! Montagem dos argumentos do yt-dlp, igual no desktop e no Android.
//!
//! A interface nunca envia argumentos prontos: ela manda só o link e as escolhas
//! (formato, qualidade, trecho) e tudo é validado aqui. Assim nenhuma opção
//! perigosa do yt-dlp (como --exec) pode chegar até ele.

use serde_json::Value;
use url::Url;

use crate::models::{DownloadRequest, Entry, Kind, MediaInfo, SavedFile};
use crate::{Error, Result};

const DOMINIOS: &[&str] = &[
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "youtu.be",
    "www.youtu.be",
    "youtube-nocookie.com",
    "www.youtube-nocookie.com",
];

/// Marcadores impressos pelo yt-dlp e lidos pelo app/interface.
pub const MARCA_PROGRESSO: &str = "@@P";
pub const MARCA_ITEM: &str = "@@I";
pub const MARCA_ARQUIVO: &str = "@@F";
pub const MARCA_CONVERTENDO: &str = "@@C";

/// Aceita só links do YouTube; devolve o link normalizado em https.
pub fn validar_url(texto: &str) -> Result<String> {
    let texto = texto.trim();
    let texto = if texto.contains("://") {
        texto.to_string()
    } else {
        format!("https://{texto}")
    };
    let mut url = Url::parse(&texto).map_err(|_| Error::msg("Link inválido."))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::msg("Link inválido."));
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !DOMINIOS.contains(&host.as_str()) {
        return Err(Error::msg("Só links do YouTube são aceitos."));
    }
    let _ = url.set_scheme("https");
    Ok(url.into())
}

/// Aceita "90", "1:30" ou "1:02:03" e devolve no formato do yt-dlp.
fn validar_tempo(texto: &str) -> Result<String> {
    let texto = texto.trim();
    let partes: Vec<&str> = texto.split(':').collect();
    let valido = !texto.is_empty()
        && partes.len() <= 3
        && partes
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()));
    if !valido {
        return Err(Error::msg(format!("Tempo inválido: \"{texto}\". Use 1:30 ou 1:02:03.")));
    }
    Ok(texto.to_string())
}

fn em_segundos(texto: &str) -> u64 {
    texto
        .split(':')
        .fold(0, |total, parte| total * 60 + parte.parse::<u64>().unwrap_or(0))
}

/// Opções comuns a todo uso do yt-dlp pelo app.
fn comuns() -> Vec<String> {
    [
        "--newline",
        "--color",
        "never",
        "--ignore-config",
        "--no-mtime",
        "--windows-filenames",
        "--trim-filenames",
        "120",
        "--retries",
        "10",
        "--fragment-retries",
        "10",
        "--extractor-retries",
        "5",
        "--concurrent-fragments",
        "4",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Argumentos para listar o conteúdo de um link (rápido, sem baixar).
pub fn args_inspecionar(url: &str, playlist: bool) -> Result<Vec<String>> {
    let url = validar_url(url)?;
    let mut a = comuns();
    a.extend(["-J", "--flat-playlist", "--no-warnings"].map(String::from));
    a.push(if playlist { "--yes-playlist" } else { "--no-playlist" }.into());
    a.extend(["--".into(), url]);
    Ok(a)
}

/// Argumentos para baixar. O caminho de saída (-P) é acrescentado por cada plataforma.
pub fn args_baixar(req: &DownloadRequest) -> Result<Vec<String>> {
    let url = validar_url(&req.url)?;
    let altura = match req.quality {
        360 | 480 | 720 | 1080 => req.quality,
        _ => 720,
    };
    let mut a = comuns();
    a.extend([
        "--no-simulate".into(),
        "--progress".into(),
        "--progress-template".into(),
        format!(
            "download:{MARCA_PROGRESSO}%(progress.{{status,downloaded_bytes,total_bytes,total_bytes_estimate,speed,eta,fragment_index,fragment_count}})j"
        ),
        "--print".into(),
        format!("before_dl:{MARCA_ITEM}%(.{{title,playlist_index,n_entries}})j"),
        "--print".into(),
        format!("post_process:{MARCA_CONVERTENDO}"),
        "--print".into(),
        format!("after_move:{MARCA_ARQUIVO}%(filepath)j"),
        "--embed-metadata".into(),
        "--embed-thumbnail".into(),
        "--convert-thumbnails".into(),
        "jpg".into(),
        // Um nome de playlist como ".." não pode levar o arquivo para fora da pasta.
        "--replace-in-metadata".into(),
        "playlist_title".into(),
        r"^\.+".into(),
        "_".into(),
    ]);

    if req.playlist {
        a.extend([
            "--yes-playlist".into(),
            "--ignore-errors".into(),
            "-o".into(),
            "%(playlist_title)s/%(playlist_index)03d - %(title)s.%(ext)s".into(),
        ]);
    } else {
        a.extend(["--no-playlist".into(), "-o".into(), "%(title)s.%(ext)s".into()]);
    }

    match req.kind {
        Kind::Video => {
            let h = format!("[height<={altura}]");
            a.extend([
                "-f".into(),
                format!(
                    "bv*{h}[vcodec^=avc1]+ba[acodec^=mp4a]/b{h}[ext=mp4][vcodec^=avc1]/bv*{h}+ba/b"
                ),
                // Só converte se o YouTube não tiver H.264 (raro).
                "--recode-video".into(),
                "mp4".into(),
            ]);
        }
        Kind::Audio => {
            a.extend(
                [
                    "-f",
                    "ba[acodec^=mp4a]/ba/b",
                    "-x",
                    "--audio-format",
                    "mp3",
                    "--audio-quality",
                    "192K",
                ]
                .map(String::from),
            );
        }
    }

    let inicio = req.start.as_deref().filter(|s| !s.trim().is_empty());
    let fim = req.end.as_deref().filter(|s| !s.trim().is_empty());
    if inicio.is_some() || fim.is_some() {
        let inicio = inicio.map(validar_tempo).transpose()?.unwrap_or_else(|| "0".into());
        let fim = fim.map(validar_tempo).transpose()?.unwrap_or_else(|| "inf".into());
        if fim != "inf" && em_segundos(&fim) <= em_segundos(&inicio) {
            return Err(Error::msg("O fim do trecho precisa ser depois do início."));
        }
        // Corte no ponto exato (sem isso ele "escorrega" até o quadro-chave anterior).
        a.extend([
            "--download-sections".into(),
            format!("*{inicio}-{fim}"),
            "--force-keyframes-at-cuts".into(),
        ]);
    }

    a.extend(["--".into(), url]);
    Ok(a)
}

/// Extrai os arquivos salvos das linhas impressas pelo yt-dlp (no Android quem lista é o Kotlin).
#[cfg_attr(mobile, allow(dead_code))]
pub fn arquivo_da_linha(linha: &str) -> Option<SavedFile> {
    let json = linha.trim().strip_prefix(MARCA_ARQUIVO)?;
    let path: String = serde_json::from_str(json).ok()?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    Some(SavedFile { name, path })
}

/// Devolve a última mensagem de erro do yt-dlp, de forma legível.
pub fn ultimo_erro(saida: &str) -> Option<String> {
    saida
        .lines()
        .rev()
        .find(|l| l.starts_with("ERROR:"))
        .map(|l| traduzir_erro(l.trim_start_matches("ERROR:").trim()))
}

pub fn traduzir_erro(erro: &str) -> String {
    let e = erro.to_lowercase();
    let dica = if e.contains("sign in to confirm") || e.contains("not a bot") {
        "O YouTube pediu confirmação de que não é um robô. Espere alguns minutos e tente de novo, ou use outra rede (ex.: Wi-Fi em vez de dados móveis)."
    } else if e.contains("private video") {
        "Esse vídeo é privado."
    } else if e.contains("video unavailable") || e.contains("not available") {
        "Esse vídeo não está disponível (pode ter sido removido ou ser bloqueado na sua região)."
    } else if e.contains("age") && e.contains("confirm") {
        "Esse vídeo tem restrição de idade e não pode ser baixado sem login."
    } else if e.contains("members-only") || e.contains("join this channel") {
        "Esse vídeo é exclusivo para membros do canal."
    } else if e.contains("live event") || e.contains("premieres in") {
        "Essa transmissão ainda não começou."
    } else if e.contains("unable to download") || e.contains("timed out") || e.contains("connection") {
        "Falha de conexão. Verifique a internet e tente de novo."
    } else {
        return erro.to_string();
    };
    dica.to_string()
}

/// Converte a saída de `yt-dlp -J --flat-playlist` em informações para a interface.
pub fn interpretar_info(json: &str) -> Result<MediaInfo> {
    let v: Value = serde_json::from_str(json.trim())
        .map_err(|_| Error::msg("Não consegui ler as informações desse link."))?;
    let texto = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(String::from);
    let miniatura = texto(&v, "thumbnail").or_else(|| {
        v.get("thumbnails")
            .and_then(Value::as_array)
            .and_then(|t| t.last())
            .and_then(|t| texto(t, "url"))
    });
    let is_playlist = v.get("_type").and_then(Value::as_str) == Some("playlist");
    let entries = v
        .get("entries")
        .and_then(Value::as_array)
        .map(|lista| {
            lista
                .iter()
                .map(|e| Entry {
                    id: texto(e, "id").unwrap_or_default(),
                    title: texto(e, "title").unwrap_or_else(|| "(sem título)".into()),
                    duration: e.get("duration").and_then(Value::as_f64),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(MediaInfo {
        title: texto(&v, "title").unwrap_or_else(|| "(sem título)".into()),
        uploader: texto(&v, "uploader").or_else(|| texto(&v, "channel")),
        thumbnail: miniatura,
        duration: v.get("duration").and_then(Value::as_f64),
        is_playlist,
        entries,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pedido(kind: Kind) -> DownloadRequest {
        DownloadRequest {
            id: "1".into(),
            url: "youtu.be/abc".into(),
            kind,
            quality: 720,
            playlist: false,
            start: None,
            end: None,
        }
    }

    #[test]
    fn aceita_so_youtube() {
        assert_eq!(validar_url("youtu.be/abc").unwrap(), "https://youtu.be/abc");
        assert!(validar_url("http://www.youtube.com/watch?v=x").unwrap().starts_with("https://"));
        for ruim in [
            "https://evil.com/youtube.com",
            "https://www.youtube.com.evil.com/x",
            "javascript://www.youtube.com",
            "file:///etc/passwd",
            "--exec rm",
            "",
        ] {
            assert!(validar_url(ruim).is_err(), "{ruim}");
        }
    }

    #[test]
    fn link_vai_depois_de_separador() {
        let a = args_baixar(&pedido(Kind::Video)).unwrap();
        assert_eq!(a[a.len() - 2], "--");
        assert!(!a.iter().any(|x| x == "--exec"));
    }

    #[test]
    fn trecho() {
        let mut p = pedido(Kind::Audio);
        p.start = Some("1:30".into());
        p.end = Some("2:00".into());
        let a = args_baixar(&p).unwrap();
        assert!(a.contains(&"*1:30-2:00".to_string()));
        p.end = Some("1:00".into());
        assert!(args_baixar(&p).is_err());
        p.end = Some("2:00; rm -rf".into());
        assert!(args_baixar(&p).is_err());
        p.start = None;
        p.end = Some("45".into());
        assert!(args_baixar(&p).unwrap().contains(&"*0-45".to_string()));
    }

    #[test]
    fn le_arquivo_e_info() {
        let f = arquivo_da_linha("@@F\"/x/Músicas/a b.mp3\"").unwrap();
        assert_eq!(f.name, "a b.mp3");
        let info = interpretar_info(
            r#"{"_type":"playlist","title":"Louvores","entries":[{"id":"a","title":"Um","duration":61.0}]}"#,
        )
        .unwrap();
        assert!(info.is_playlist);
        assert_eq!(info.entries.len(), 1);
    }
}
