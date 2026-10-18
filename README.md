# ytdownload

Baixa vídeos e playlists do YouTube em **MP4 e MP3 ao mesmo tempo**, para assistir
ou ouvir sem internet.

Este repositório tem duas formas de uso:

- **App com interface (Android, Windows e Linux):** veja [`app/README.md`](app/README.md).
  Baixe o APK/instalador na página [Releases](../../releases).
- **Script em lista (este README):** cole vários links num arquivo e baixe tudo de uma vez.

## Como usar

1. Cole os links em **`links.txt`**, um por linha. Pode ser vídeo, playlist ou canal.
2. Rode:
   - **Windows:** dois cliques em **`baixar.bat`**
   - **Linux / macOS:** `./baixar.sh`
3. Pegue os arquivos na pasta **`downloads/`**:

```
downloads/
├── MP4/
│   ├── Avulsos/                     ← vídeos soltos
│   └── Nome da Playlist/            ← uma pasta por playlist, na ordem dela
│       ├── 001 - Primeira música [id].mp4
│       └── 002 - Segunda música [id].mp4
└── MP3/  (mesma organização, com a capa do vídeo embutida)
```

Na primeira vez o script prepara tudo sozinho (só precisa do
[Python 3.10+](https://www.python.org/downloads/) instalado; no Windows marque
*"Add python.exe to PATH"*). Pode rodar quantas vezes quiser: **o que já foi
baixado é pulado**, então dá para ir adicionando links ao `links.txt` aos poucos.
Se algum link falhar, ele fica listado em `downloads/falhas.txt` — é só rodar de novo depois.

### Sem computador? Rode na nuvem do GitHub

Pelo site do GitHub (funciona no celular): edite o `links.txt`, vá em
**Actions → Baixar vídeos → Run workflow**, e ao terminar baixe o `.zip` em
**Artifacts** (fica disponível por 7 dias). Observação: o YouTube às vezes bloqueia
servidores de nuvem; se der erro de "não é um robô", rode no computador.

## Qual formato usar?

| Formato | Funciona em | Observação |
|---|---|---|
| **MP4** (H.264 + AAC) | TVs, celulares, tablets, computadores, pen drive na TV | O mais universal. Também toca só o som na maioria dos celulares e players. |
| **MP3** | Praticamente tudo que toca áudio: som do carro, caixinha de som, MP3 player, rádio com USB | Só áudio. |

Não existe um único arquivo que funcione em 100% dos aparelhos: o MP4 é o "faz
tudo" mais próximo, mas muitos sons de carro e caixinhas USB só leem MP3. Por isso
o padrão é baixar **os dois**.

## Opções

```
baixar.bat -f mp3                # só MP3   (ou -f mp4 para só vídeo)
baixar.bat -q 480                # vídeo menor (360, 480, 720 [padrão], 1080)
baixar.bat --kbps 128            # MP3 menor (128, 192 [padrão], 256, 320)
baixar.bat https://youtu.be/...  # links direto, sem usar o links.txt
baixar.bat --playlist-inteira    # link de vídeo "dentro" de playlist (watch?v=...&list=...)
                                 # baixa a playlist toda (padrão: só o vídeo)
```
(No Linux/macOS, troque `baixar.bat` por `./baixar.sh`.)

## Confiabilidade e segurança

- Usa o [**yt-dlp**](https://github.com/yt-dlp/yt-dlp), o baixador de código aberto
  mais usado e mantido; ele é **atualizado automaticamente** a cada execução, porque
  o YouTube muda com frequência e versões antigas param de funcionar.
- Usa **ffmpeg** para converter e **Deno** (exigido pelo yt-dlp para o YouTube),
  ambos instalados pelo `pip`, dentro de uma pasta isolada (`.venv`).
- **Só aceita links do YouTube**; qualquer outro endereço é ignorado.
- Não usa login, senha nem cookies do navegador.
- Nomes de arquivo são limpos para funcionar em Windows, Mac, Linux e pen drives,
  e nunca são gravados fora da pasta `downloads/`.
- Tenta de novo automaticamente em falhas de rede e faz pausas curtas entre vídeos
  para evitar bloqueio.

Use para conteúdo que você tem direito de baixar, respeitando os termos do YouTube.
