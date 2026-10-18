//! Motor no computador: usa os executáveis oficiais do yt-dlp, do Deno e do ffmpeg,
//! baixados do GitHub na primeira vez e conferidos pelo SHA-256 publicado por cada projeto.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use tauri::ipc::Channel;
use tauri::{plugin::PluginApi, AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use crate::args;
use crate::models::*;
use crate::{Error, Result};

const YTDLP_RELEASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest";
const FFMPEG_RELEASE: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest";
const DENO_RELEASE: &str = "https://github.com/denoland/deno/releases/latest/download";
/// O YouTube muda com frequência; o yt-dlp é atualizado se tiver mais que isso.
const IDADE_MAXIMA_YTDLP: Duration = Duration::from_secs(2 * 24 * 3600);

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<Ytdlp<R>> {
    Ok(Ytdlp {
        app: app.clone(),
        tarefas: Mutex::new(HashMap::new()),
    })
}

pub struct Ytdlp<R: Runtime> {
    app: AppHandle<R>,
    /// Downloads em andamento, para poder cancelar.
    tarefas: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

fn exe(nome: &str) -> String {
    if cfg!(windows) {
        format!("{nome}.exe")
    } else {
        nome.to_string()
    }
}

fn procurar_no_path(nome: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(exe(nome)))
        .find(|p| p.is_file())
}

fn asset_ytdlp() -> &'static str {
    if cfg!(windows) {
        if cfg!(target_arch = "aarch64") {
            "yt-dlp_arm64.exe"
        } else {
            "yt-dlp.exe"
        }
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else if cfg!(target_arch = "aarch64") {
        "yt-dlp_linux_aarch64"
    } else {
        "yt-dlp_linux"
    }
}

fn asset_deno() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => "deno-x86_64-pc-windows-msvc.zip",
        ("macos", "aarch64") => "deno-aarch64-apple-darwin.zip",
        ("macos", _) => "deno-x86_64-apple-darwin.zip",
        (_, "aarch64") => "deno-aarch64-unknown-linux-gnu.zip",
        _ => "deno-x86_64-unknown-linux-gnu.zip",
    }
}

fn asset_ffmpeg() -> Option<&'static str> {
    if !cfg!(windows) {
        return None; // Linux e macOS: usa o ffmpeg do sistema
    }
    Some(if cfg!(target_arch = "aarch64") {
        "ffmpeg-master-latest-winarm64-gpl.zip"
    } else {
        "ffmpeg-master-latest-win64-gpl.zip"
    })
}

/// Encontra o SHA-256 de `nome` num arquivo de checksums (formato "hash  nome",
/// ou o formato do PowerShell que o Deno usa no Windows).
fn achar_hash(texto: &str, nome: &str) -> Option<String> {
    let eh_hash = |t: &str| t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit());
    let por_nome = texto.lines().find_map(|linha| {
        let mut partes = linha.split_whitespace();
        let hash = partes.next()?;
        let arquivo = partes.last()?.trim_start_matches('*');
        (eh_hash(hash) && arquivo == nome).then(|| hash.to_ascii_lowercase())
    });
    por_nome.or_else(|| {
        let hashes: Vec<&str> = texto.split_whitespace().filter(|t| eh_hash(t)).collect();
        (hashes.len() == 1).then(|| hashes[0].to_ascii_lowercase())
    })
}

fn cliente() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("YTDownload/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| Error::msg(format!("Falha ao iniciar conexão: {e}")))
}

fn falha_rede(e: reqwest::Error) -> Error {
    Error::msg(format!("Falha ao baixar o motor (verifique a internet): {e}"))
}

/// Baixa `url` para `destino`, conferindo o SHA-256 antes de aceitar o arquivo.
async fn baixar_verificado(
    url: &str,
    sha256_esperado: &str,
    destino: &Path,
    rotulo: &str,
    canal: &Channel<Event>,
) -> Result<()> {
    let resposta = cliente()?
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(falha_rede)?;
    let total = resposta.content_length();
    let temporario = destino.with_extension("parcial");
    let mut arquivo = tokio::fs::File::create(&temporario).await?;
    let mut hasher = Sha256::new();
    let mut baixado: u64 = 0;
    let mut ultimo_aviso = 0.0;
    let mut fluxo = resposta.bytes_stream();
    while let Some(pedaco) = fluxo.next().await {
        let pedaco = pedaco.map_err(falha_rede)?;
        hasher.update(&pedaco);
        arquivo.write_all(&pedaco).await?;
        baixado += pedaco.len() as u64;
        if let Some(total) = total {
            let p = baixado as f64 / total as f64;
            if p - ultimo_aviso >= 0.02 || baixado == total {
                ultimo_aviso = p;
                let _ = canal.send(Event::Stage {
                    message: format!("Baixando {rotulo}…"),
                    progress: Some(p),
                });
            }
        }
    }
    arquivo.flush().await?;
    drop(arquivo);
    let obtido = format!("{:x}", hasher.finalize());
    if obtido != sha256_esperado {
        let _ = tokio::fs::remove_file(&temporario).await;
        return Err(Error::msg(format!(
            "O arquivo do {rotulo} não passou na verificação de segurança (SHA-256). Nada foi instalado."
        )));
    }
    tokio::fs::rename(&temporario, destino).await?;
    Ok(())
}

async fn baixar_texto(url: &str) -> Result<String> {
    cliente()?
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(falha_rede)?
        .text()
        .await
        .map_err(falha_rede)
}

#[cfg(unix)]
fn tornar_executavel(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}
#[cfg(not(unix))]
fn tornar_executavel(_: &Path) -> Result<()> {
    Ok(())
}

/// Extrai do zip os arquivos cujo nome termina com algum de `nomes`.
fn extrair(zip_path: &Path, nomes: &[String], pasta: &Path) -> Result<()> {
    let arquivo = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(arquivo).map_err(|e| Error::msg(format!("Zip inválido: {e}")))?;
    let mut faltando: Vec<&String> = nomes.iter().collect();
    for i in 0..zip.len() {
        let mut item = zip.by_index(i).map_err(|e| Error::msg(e.to_string()))?;
        let nome_item = item.name().replace('\\', "/");
        let base = nome_item.rsplit('/').next().unwrap_or_default().to_string();
        if let Some(pos) = faltando.iter().position(|n| **n == base) {
            let destino = pasta.join(&base);
            let mut saida = std::fs::File::create(&destino)?;
            std::io::copy(&mut item, &mut saida)?;
            tornar_executavel(&destino)?;
            faltando.remove(pos);
        }
    }
    if !faltando.is_empty() {
        return Err(Error::msg("Pacote baixado não contém os arquivos esperados."));
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Config {
    output_dir: Option<PathBuf>,
}

/// Lê a saída de um processo linha a linha (tolerando texto que não seja UTF-8).
async fn ler_linhas(
    leitor: impl AsyncRead + Unpin,
    canal: Option<Channel<Event>>,
) -> String {
    let mut leitor = BufReader::new(leitor);
    let mut tudo = String::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match leitor.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let linha = String::from_utf8_lossy(&buf);
                let linha = linha.trim_end_matches(['\r', '\n']);
                if let Some(c) = &canal {
                    let _ = c.send(Event::Line { line: linha.to_string() });
                }
                tudo.push_str(linha);
                tudo.push('\n');
            }
        }
    }
    tudo
}

impl<R: Runtime> Ytdlp<R> {
    fn pasta_motor(&self) -> Result<PathBuf> {
        let p = self.app.path().app_local_data_dir()?.join("motor");
        std::fs::create_dir_all(&p)?;
        Ok(p)
    }

    fn caminho_ytdlp(&self) -> Result<PathBuf> {
        Ok(self.pasta_motor()?.join(exe("yt-dlp")))
    }

    fn caminho_deno(&self) -> Result<Option<PathBuf>> {
        let local = self.pasta_motor()?.join(exe("deno"));
        Ok(local.is_file().then_some(local).or_else(|| procurar_no_path("deno")))
    }

    /// `Ok(None)` = usar o ffmpeg do sistema.
    fn pasta_ffmpeg(&self) -> Result<Option<Option<PathBuf>>> {
        let local = self.pasta_motor()?.join("ffmpeg");
        if local.join(exe("ffmpeg")).is_file() && local.join(exe("ffprobe")).is_file() {
            return Ok(Some(Some(local)));
        }
        if procurar_no_path("ffmpeg").is_some() && procurar_no_path("ffprobe").is_some() {
            return Ok(Some(None));
        }
        Ok(None)
    }

    fn arquivo_config(&self) -> Result<PathBuf> {
        let p = self.app.path().app_config_dir()?;
        std::fs::create_dir_all(&p)?;
        Ok(p.join("config.json"))
    }

    fn ler_config(&self) -> Config {
        self.arquivo_config()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn pasta_saida(&self) -> Result<PathBuf> {
        match self.ler_config().output_dir {
            Some(p) => Ok(p),
            None => {
                let caminhos = self.app.path();
                let base = caminhos
                    .download_dir()
                    .or_else(|_| caminhos.home_dir().map(|h| h.join("Downloads")))
                    .or_else(|_| caminhos.app_local_data_dir())?;
                Ok(base.join("YTDownload"))
            }
        }
    }

    fn subpasta(&self, kind: Kind) -> Result<PathBuf> {
        Ok(self.pasta_saida()?.join(match kind {
            Kind::Video => "Vídeos",
            Kind::Audio => "Áudios",
        }))
    }

    async fn versao(&self) -> Option<String> {
        let caminho = self.caminho_ytdlp().ok()?;
        if !caminho.is_file() {
            return None;
        }
        let mut cmd = Command::new(caminho);
        cmd.arg("--version");
        sem_janela(&mut cmd);
        let saida = cmd.output().await.ok()?;
        Some(String::from_utf8_lossy(&saida.stdout).trim().to_string()).filter(|v| !v.is_empty())
    }

    pub async fn status(&self) -> Result<EngineStatus> {
        let pronto = self.caminho_ytdlp()?.is_file()
            && self.caminho_deno()?.is_some()
            && self.pasta_ffmpeg()?.is_some();
        Ok(EngineStatus {
            ready: pronto,
            version: self.versao().await,
            output_dir: Some(self.pasta_saida()?.to_string_lossy().into_owned()),
            platform: std::env::consts::OS.into(),
        })
    }

    async fn instalar_ytdlp(&self, canal: &Channel<Event>) -> Result<()> {
        let nome = asset_ytdlp();
        let somas = baixar_texto(&format!("{YTDLP_RELEASE}/download/SHA2-256SUMS")).await?;
        let hash = achar_hash(&somas, nome)
            .ok_or_else(|| Error::msg("Não encontrei o checksum do yt-dlp."))?;
        let destino = self.caminho_ytdlp()?;
        baixar_verificado(&format!("{YTDLP_RELEASE}/download/{nome}"), &hash, &destino, "yt-dlp", canal)
            .await?;
        tornar_executavel(&destino)
    }

    async fn instalar_deno(&self, canal: &Channel<Event>) -> Result<()> {
        let nome = asset_deno();
        let somas = baixar_texto(&format!("{DENO_RELEASE}/{nome}.sha256sum")).await?;
        let hash = achar_hash(&somas, nome).ok_or_else(|| Error::msg("Não encontrei o checksum do Deno."))?;
        let pasta = self.pasta_motor()?;
        let zip = pasta.join("deno.zip");
        baixar_verificado(&format!("{DENO_RELEASE}/{nome}"), &hash, &zip, "Deno", canal).await?;
        let r = extrair(&zip, &[exe("deno")], &pasta);
        let _ = std::fs::remove_file(&zip);
        r
    }

    async fn instalar_ffmpeg(&self, canal: &Channel<Event>) -> Result<()> {
        let Some(nome) = asset_ffmpeg() else {
            return Err(Error::msg(if cfg!(target_os = "macos") {
                "Instale o ffmpeg com: brew install ffmpeg — e abra o app de novo."
            } else {
                "Instale o ffmpeg com: sudo apt install ffmpeg (ou o gerenciador da sua distribuição) — e abra o app de novo."
            }));
        };
        let somas = baixar_texto(&format!("{FFMPEG_RELEASE}/checksums.sha256")).await?;
        let hash = achar_hash(&somas, nome).ok_or_else(|| Error::msg("Não encontrei o checksum do ffmpeg."))?;
        let pasta = self.pasta_motor()?.join("ffmpeg");
        std::fs::create_dir_all(&pasta)?;
        let zip = pasta.join("ffmpeg.zip");
        baixar_verificado(&format!("{FFMPEG_RELEASE}/{nome}"), &hash, &zip, "ffmpeg", canal).await?;
        let r = extrair(&zip, &[exe("ffmpeg"), exe("ffprobe")], &pasta);
        let _ = std::fs::remove_file(&zip);
        r
    }

    /// Instala o que faltar e atualiza o yt-dlp se estiver velho.
    pub async fn prepare(&self, canal: Channel<Event>) -> Result<EngineStatus> {
        let etapa = |m: &str| {
            let _ = canal.send(Event::Stage { message: m.into(), progress: None });
        };
        let ytdlp = self.caminho_ytdlp()?;
        if !ytdlp.is_file() {
            etapa("Instalando o yt-dlp…");
            self.instalar_ytdlp(&canal).await?;
        } else {
            let velho = std::fs::metadata(&ytdlp)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|idade| idade > IDADE_MAXIMA_YTDLP);
            if velho {
                etapa("Procurando atualização do yt-dlp…");
                // Sem internet ou GitHub fora do ar: segue com a versão atual.
                let _ = self.update_com(&canal).await;
            }
        }
        if self.caminho_deno()?.is_none() {
            etapa("Instalando o Deno…");
            self.instalar_deno(&canal).await?;
        }
        if self.pasta_ffmpeg()?.is_none() {
            etapa("Instalando o ffmpeg…");
            self.instalar_ffmpeg(&canal).await?;
        }
        self.status().await
    }

    async fn update_com(&self, canal: &Channel<Event>) -> Result<String> {
        let atual = self.versao().await;
        // /releases/latest redireciona para /releases/tag/<versão>.
        let resposta = cliente()?.get(YTDLP_RELEASE).send().await.map_err(falha_rede)?;
        let ultima = resposta.url().path_segments().and_then(|s| s.last()).map(String::from);
        if atual.is_some() && atual == ultima {
            // Marca como conferido agora, para não checar a cada abertura.
            let _ = std::fs::File::options()
                .append(true)
                .open(self.caminho_ytdlp()?)
                .and_then(|f| f.set_modified(SystemTime::now()));
        } else {
            self.instalar_ytdlp(canal).await?;
        }
        Ok(self.versao().await.unwrap_or_default())
    }

    pub async fn update(&self) -> Result<String> {
        self.update_com(&Channel::new(|_| Ok(()))).await
    }

    /// Roda o yt-dlp e devolve (código de saída, stdout, stderr).
    async fn executar(
        &self,
        argumentos: Vec<String>,
        pasta: Option<&Path>,
        id: Option<&str>,
        canal: Option<Channel<Event>>,
    ) -> Result<(Option<i32>, String, String)> {
        let ytdlp = self.caminho_ytdlp()?;
        if !ytdlp.is_file() {
            return Err(Error::msg("O motor ainda não foi instalado."));
        }
        let mut cmd = Command::new(ytdlp);
        cmd.arg("--cache-dir").arg(self.pasta_motor()?.join("cache"));
        if let Some(deno) = self.caminho_deno()? {
            cmd.arg("--js-runtimes").arg(format!("deno:{}", deno.display()));
        }
        if let Some(Some(ff)) = self.pasta_ffmpeg()? {
            cmd.arg("--ffmpeg-location").arg(ff);
        }
        if let Some(p) = pasta {
            std::fs::create_dir_all(p)?;
            cmd.arg("-P").arg(p);
        }
        cmd.args(&argumentos)
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUTF8", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        sem_janela(&mut cmd);

        let mut filho = cmd.spawn()?;
        let saida = tokio::spawn(ler_linhas(filho.stdout.take().unwrap(), canal.clone()));
        let erros = tokio::spawn(ler_linhas(filho.stderr.take().unwrap(), canal));

        let (tx, rx) = oneshot::channel();
        if let Some(id) = id {
            self.tarefas.lock().unwrap().insert(id.to_string(), tx);
        }
        let cancelado = tokio::select! {
            r = filho.wait() => { r?; false }
            _ = rx => { let _ = filho.kill().await; true }
        };
        if let Some(id) = id {
            self.tarefas.lock().unwrap().remove(id);
        }
        let codigo = filho.wait().await?.code();
        let saida = saida.await.unwrap_or_default();
        let erros = erros.await.unwrap_or_default();
        if cancelado {
            return Err(Error::msg("Cancelado."));
        }
        Ok((codigo, saida, erros))
    }

    pub async fn inspect(&self, url: String, playlist: bool) -> Result<MediaInfo> {
        let (codigo, saida, erros) = self
            .executar(args::args_inspecionar(&url, playlist)?, None, None, None)
            .await?;
        if codigo != Some(0) && saida.trim().is_empty() {
            return Err(Error::msg(
                args::ultimo_erro(&erros).unwrap_or_else(|| "Não consegui abrir esse link.".into()),
            ));
        }
        args::interpretar_info(&saida)
    }

    pub async fn download(&self, req: DownloadRequest, canal: Channel<Event>) -> Result<DownloadResult> {
        let argumentos = args::args_baixar(&req)?;
        let pasta = self.subpasta(req.kind)?;
        let (codigo, saida, erros) = self
            .executar(argumentos, Some(&pasta), Some(&req.id), Some(canal))
            .await?;
        let files: Vec<SavedFile> = saida.lines().filter_map(args::arquivo_da_linha).collect();
        let erro = args::ultimo_erro(&erros);
        if codigo == Some(0) || !files.is_empty() {
            Ok(DownloadResult {
                files,
                error: if codigo == Some(0) { None } else { erro },
            })
        } else {
            Err(Error::msg(erro.unwrap_or_else(|| "O download falhou.".into())))
        }
    }

    pub fn cancel(&self, id: String) -> Result<()> {
        if let Some(tx) = self.tarefas.lock().unwrap().remove(&id) {
            let _ = tx.send(());
        }
        Ok(())
    }

    /// Só abre arquivos que estão dentro da pasta de downloads do app.
    fn conferir_caminho(&self, caminho: &str) -> Result<PathBuf> {
        let base = self.pasta_saida()?.canonicalize()?;
        let alvo = Path::new(caminho).canonicalize()?;
        if !alvo.starts_with(&base) {
            return Err(Error::msg("Arquivo fora da pasta de downloads."));
        }
        Ok(alvo)
    }

    pub fn open_file(&self, path: String) -> Result<()> {
        let alvo = self.conferir_caminho(&path)?;
        self.app
            .opener()
            .open_path(alvo.to_string_lossy(), None::<&str>)
            .map_err(|e| Error::msg(e.to_string()))
    }

    pub fn reveal_file(&self, path: String) -> Result<()> {
        let alvo = self.conferir_caminho(&path)?;
        self.app.opener().reveal_item_in_dir(alvo).map_err(|e| Error::msg(e.to_string()))
    }

    pub fn share_file(&self, path: String) -> Result<()> {
        self.reveal_file(path)
    }

    pub fn open_output_dir(&self) -> Result<()> {
        let pasta = self.pasta_saida()?;
        std::fs::create_dir_all(&pasta)?;
        self.app
            .opener()
            .open_path(pasta.to_string_lossy(), None::<&str>)
            .map_err(|e| Error::msg(e.to_string()))
    }

    pub async fn choose_output_dir(&self) -> Result<EngineStatus> {
        let app = self.app.clone();
        let escolha = tokio::task::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|e| Error::msg(e.to_string()))?;
        if let Some(pasta) = escolha.and_then(|p| p.into_path().ok()) {
            let config = Config { output_dir: Some(pasta) };
            std::fs::write(self.arquivo_config()?, serde_json::to_string_pretty(&config).unwrap())?;
        }
        self.status().await
    }

    pub fn take_shared_url(&self) -> Result<SharedUrl> {
        Ok(SharedUrl::default())
    }

    /// No computador a interface lê a área de transferência sozinha.
    pub fn read_clipboard(&self) -> Result<SharedUrl> {
        Ok(SharedUrl::default())
    }
}

/// No Windows, impede que uma janela preta de terminal apareça.
fn sem_janela(cmd: &mut Command) {
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    #[cfg(not(windows))]
    let _ = cmd;
}

#[cfg(test)]
mod testes {
    use super::achar_hash;

    #[test]
    fn checksums() {
        let h = "a".repeat(64);
        assert_eq!(achar_hash(&format!("{h}  yt-dlp.exe\n{}  yt-dlp\n", "b".repeat(64)), "yt-dlp.exe"), Some(h.clone()));
        let ps = format!("\nAlgorithm : SHA256\nHash      : {}\nPath      : C:\\a\\deno.zip\n", h.to_uppercase());
        assert_eq!(achar_hash(&ps, "deno-x86_64-pc-windows-msvc.zip"), Some(h));
        assert_eq!(achar_hash("nada aqui", "x"), None);
    }
}

/// Teste real (precisa de internet e de um servidor local com /teste.webm):
/// YTDLP_TESTE_URL=http://127.0.0.1:8765/teste.webm cargo test -- --ignored --nocapture
#[cfg(test)]
mod teste_integracao {
    use super::*;
    use std::sync::{Arc, Mutex as M};

    #[tokio::test]
    #[ignore]
    async fn instala_e_baixa() {
        let url_local = std::env::var("YTDLP_TESTE_URL").expect("defina YTDLP_TESTE_URL");
        let app = tauri::test::mock_app();
        let motor = Ytdlp { app: app.handle().clone(), tarefas: Mutex::new(HashMap::new()) };

        let etapas = Arc::new(M::new(Vec::<String>::new()));
        let e2 = etapas.clone();
        let canal = Channel::new(move |b| {
            if let tauri::ipc::InvokeResponseBody::Json(j) = b {
                e2.lock().unwrap().push(j);
            }
            Ok(())
        });
        let status = motor.prepare(canal).await.expect("prepare");
        println!("status: ready={} versão={:?}", status.ready, status.version);
        assert!(status.ready);

        for (kind, audio_format) in [
            (Kind::Video, AudioFormat::Mp3),
            (Kind::Audio, AudioFormat::Mp3),
            (Kind::Audio, AudioFormat::M4a),
        ] {
            let req = DownloadRequest {
                id: format!("t-{kind:?}-{audio_format:?}"),
                url: "https://youtu.be/x".into(),
                kind,
                quality: 720,
                playlist: false,
                audio_format,
                start: Some("0:01".into()),
                end: Some("0:03".into()),
            };
            let mut a = args::args_baixar(&req).unwrap();
            *a.last_mut().unwrap() = url_local.clone(); // troca o link do YouTube pelo local
            let linhas = Arc::new(M::new(Vec::<String>::new()));
            let l2 = linhas.clone();
            let canal = Channel::new(move |b| {
                if let tauri::ipc::InvokeResponseBody::Json(j) = b {
                    l2.lock().unwrap().push(j);
                }
                Ok(())
            });
            let pasta = std::env::temp_dir().join("ytdlp-teste").join(&req.id);
            let _ = std::fs::remove_dir_all(&pasta);
            let (codigo, saida, erros) = motor.executar(a, Some(&pasta), Some(&req.id), Some(canal)).await.unwrap();
            let arquivos: Vec<_> = saida.lines().filter_map(args::arquivo_da_linha).collect();
            let linhas = linhas.lock().unwrap();
            println!("{kind:?}: código={codigo:?} arquivos={arquivos:?}");
            println!("  marcadores: P={} I={} C={}", 
                linhas.iter().filter(|l| l.contains("@@P")).count(),
                linhas.iter().filter(|l| l.contains("@@I")).count(),
                linhas.iter().filter(|l| l.contains("@@C")).count());
            if codigo != Some(0) { println!("{erros}"); }
            assert_eq!(codigo, Some(0));
            assert_eq!(arquivos.len(), 1);
            assert!(Path::new(&arquivos[0].path).is_file());
        }
    }
}
