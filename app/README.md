# YTDownload — app (Android + Windows + Linux)

App com interface para baixar vídeos (MP4) e áudios (MP3) do YouTube direto no
aparelho, sem passar por sites intermediários como o Cobalt. O download sai do
YouTube direto para o seu celular ou computador, então é tão rápido quanto a sua internet.

## Instalar

Abra a página **[Releases](../../releases)** do repositório e baixe:

| Aparelho | Arquivo |
|---|---|
| Celular Android | `YTDownload-android-arm64.apk` (se não instalar, use o `arm32`) |
| Windows | `YTDownload-windows-instalador.exe` |
| Linux | `YTDownload-linux.AppImage` ou `.deb` (precisa do `ffmpeg` instalado) |

No Android, ao abrir o APK o celular pede para permitir "instalar apps desta fonte": permita.
O Windows pode mostrar "O Windows protegeu o computador" (o app não tem certificado pago):
clique em **Mais informações → Executar assim mesmo**.

Para gerar uma versão nova: **Actions → App (Android + desktop) → Run workflow**.
Em uns 20 minutos ela aparece em Releases.

## Usar

- **Pelo YouTube (Android):** no vídeo, toque em **Compartilhar → YTDownload**. O app abre com o link pronto.
- **Colando o link:** toque em **Colar**. O app mostra título, duração e, se for playlist, a lista de vídeos.
- Escolha **Vídeo (MP4)**, **Áudio (MP3)** ou **Os dois**, a qualidade, e toque em **Baixar**.
- **Só um trecho:** marque "Baixar só um trecho" e informe início e fim (ex.: `12:30` até `15:00`).
  Útil para pegar só a música ou só a pregação.
- Pode colocar vários downloads na fila; dois baixam ao mesmo tempo.
- Quando terminar: **▶ Abrir** ou **Enviar** (WhatsApp, Drive, Bluetooth…).

Onde ficam os arquivos:
- Android: pasta **Download/YTDownload/Vídeos** e **Download/YTDownload/Áudios** (aparecem na galeria e no app Arquivos).
- Computador: **Downloads/YTDownload** (dá para trocar em ⚙ Configurações).

## Como funciona (e por que é confiável)

- O motor é o [**yt-dlp**](https://github.com/yt-dlp/yt-dlp), o mesmo usado por trás de quase
  todo baixador sério. Ele se atualiza sozinho a cada 2 dias, porque o YouTube muda com frequência.
  Se algo parar de funcionar, ⚙ → **Atualizar agora**.
- **Android:** usa a biblioteca [youtubedl-android](https://github.com/JunkFood02/youtubedl-android)
  (a mesma do app Seal), que traz yt-dlp, Python, ffmpeg e QuickJS dentro do APK.
- **Windows/Linux:** na primeira vez o app baixa os programas oficiais do yt-dlp, Deno e ffmpeg
  (no Linux usa o ffmpeg do sistema) direto do GitHub de cada projeto, e confere o SHA-256 de
  cada arquivo antes de usar. Se a conferência falhar, nada é instalado.
- Feito com [Tauri 2](https://tauri.app) (Rust) + Svelte: leve e rápido, sem Electron.

### Segurança

- Só aceita links do YouTube.
- A interface nunca monta comandos: ela envia só o link e as escolhas, e o lado Rust valida tudo
  (inclusive os tempos do trecho) antes de chamar o yt-dlp, sempre com o link depois de `--`.
- Não usa login, senha nem cookies.
- Nomes de arquivo são limpos e nunca saem da pasta de downloads.
- No computador, "abrir arquivo" só funciona para arquivos dentro da pasta de downloads do app.

## Assinatura do APK (para atualizar sem desinstalar)

Sem configurar nada, cada versão do APK sai com uma chave diferente, e o Android exige
desinstalar a versão antiga antes de instalar a nova (os arquivos baixados não são apagados).
Para evitar isso, crie uma chave **uma vez** num computador com Java:

```bash
keytool -genkeypair -v -keystore ytdownload.jks -alias ytdownload -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 ytdownload.jks > ytdownload.jks.base64   # no macOS: base64 -i ytdownload.jks
```

E em **Settings → Secrets and variables → Actions** do repositório, crie:

| Secret | Valor |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | conteúdo de `ytdownload.jks.base64` |
| `ANDROID_KEYSTORE_PASSWORD` | a senha escolhida |
| `ANDROID_KEY_ALIAS` | `ytdownload` |

Guarde o `ytdownload.jks` e a senha em lugar seguro e **nunca** coloque no repositório.

## Desenvolvimento

```bash
cd app
npm install
npm run tauri dev            # desktop
npm run tauri android dev    # Android (precisa do Android SDK/NDK)
cd plugins/ytdlp && cargo test
```

Estrutura:
- `src/` — interface (Svelte)
- `plugins/ytdlp/src/args.rs` — montagem e validação dos argumentos do yt-dlp (comum às plataformas)
- `plugins/ytdlp/src/desktop.rs` — motor no computador
- `plugins/ytdlp/android/` — motor no Android (Kotlin)
- `src-tauri/gen/android/` — projeto Android (manifesto com o "Compartilhar", assinatura)
