#!/usr/bin/env python3
"""Baixa vídeos e playlists do YouTube em MP4 e/ou MP3.

Uso rápido:
    python baixar.py                      # lê os links de links.txt e baixa MP4 + MP3
    python baixar.py --formato mp3        # só MP3
    python baixar.py URL1 URL2            # links direto na linha de comando

Os arquivos vão para a pasta "downloads/", separados em MP4/ e MP3/.
Rodar de novo é seguro: o que já foi baixado é pulado.
"""

from __future__ import annotations

import argparse
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path
from urllib.parse import parse_qs, urlparse

try:
    import yt_dlp
    from yt_dlp.postprocessor import MetadataParserPP
except ImportError:
    sys.exit("Falta instalar as dependências. Rode: pip install -r requirements.txt")

PASTA_DO_SCRIPT = Path(__file__).resolve().parent

# Só aceitamos links do YouTube. Qualquer outro endereço é recusado,
# para que um link estranho colado por engano não leve a outro site.
DOMINIOS_PERMITIDOS = {
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "youtu.be",
    "www.youtu.be",
    "youtube-nocookie.com",
    "www.youtube-nocookie.com",
}

# MP4 com vídeo H.264 + áudio AAC: a combinação que mais aparelhos
# (TVs, celulares, tablets, pen drive no som do carro com tela) reproduzem.
def formato_mp4(altura_max: int) -> str:
    h = f"[height<={altura_max}]"
    return "/".join([
        f"bv*{h}[vcodec^=avc1]+ba[acodec^=mp4a]",  # ideal: H.264 + AAC
        f"b{h}[ext=mp4][vcodec^=avc1]",             # arquivo único já compatível
        f"bv*{h}+ba",                               # qualquer codec (será convertido)
        "b",
    ])


@dataclass
class Resultado:
    ok: list[str] = field(default_factory=list)
    falhas: list[str] = field(default_factory=list)


def validar_link(texto: str) -> str | None:
    """Devolve o link normalizado se for do YouTube, senão None."""
    texto = texto.strip()
    if not texto:
        return None
    if "://" not in texto:
        texto = "https://" + texto
    try:
        url = urlparse(texto)
    except ValueError:
        return None
    if url.scheme not in ("http", "https"):
        return None
    if (url.hostname or "").lower() not in DOMINIOS_PERMITIDOS:
        return None
    return url._replace(scheme="https").geturl()


def eh_playlist(link: str) -> bool:
    url = urlparse(link)
    return url.path.rstrip("/") == "/playlist" and "list" in parse_qs(url.query)


def ler_links(arquivo: Path) -> list[str]:
    if not arquivo.exists():
        sys.exit(f"Arquivo de links não encontrado: {arquivo}")
    linhas = arquivo.read_text(encoding="utf-8-sig").splitlines()
    # Ignora linhas vazias e comentários (#). Permite texto depois do link,
    # ex.: "https://youtu.be/xxxx  música do sapo".
    return [l.split()[0] for l in linhas if l.strip() and not l.lstrip().startswith("#")]


def localizar_ffmpeg() -> str | None:
    """Usa o ffmpeg do sistema; se não houver, usa o empacotado (static-ffmpeg)."""
    if shutil.which("ffmpeg") and shutil.which("ffprobe"):
        return None  # yt-dlp encontra sozinho
    try:
        import static_ffmpeg.run
        ffmpeg, _ffprobe = static_ffmpeg.run.get_or_fetch_platform_executables_else_raise()
        return str(Path(ffmpeg).parent)
    except Exception as erro:  # noqa: BLE001
        sys.exit(
            "Não encontrei o ffmpeg (necessário para gerar MP3/MP4).\n"
            "Instale com: pip install -r requirements.txt  (ou instale o ffmpeg no sistema)\n"
            f"Detalhe: {erro}"
        )


def localizar_deno() -> dict:
    """O YouTube exige um interpretador JavaScript; usamos o Deno instalado via pip."""
    try:
        import deno
        return {"deno": {"path": deno.find_deno_bin()}}
    except Exception:  # noqa: BLE001
        return {"deno": {}}  # tenta o Deno do sistema, se houver


def opcoes_base(destino: Path, arquivo_registro: Path, ffmpeg_dir: str | None,
                playlist_inteira: bool) -> dict:
    # Uma pasta por playlist, com os arquivos numerados na ordem dela;
    # vídeos soltos vão para a pasta "Avulsos".
    modelo = "%(playlist_title|Avulsos)s/%(playlist_index&{:03d} - |)s%(title)s [%(id)s].%(ext)s"
    opcoes = {
        "outtmpl": {"default": modelo},
        "paths": {"home": str(destino), "temp": str(destino / ".temp")},
        "windowsfilenames": True,       # nomes válidos em Windows, Mac, Linux e pen drives
        "trim_file_name": 120,
        "download_archive": str(arquivo_registro),  # não baixa de novo o que já tem
        "noplaylist": not playlist_inteira,
        "ignoreerrors": "only_download",  # um vídeo com problema não para os outros
        "retries": 10,
        "fragment_retries": 10,
        "extractor_retries": 5,
        "file_access_retries": 5,
        "sleep_interval": 2,            # pausa entre vídeos para não ser bloqueado
        "max_sleep_interval": 6,
        "sleep_interval_requests": 1,
        "concurrent_fragment_downloads": 3,
        "overwrites": False,
        "writethumbnail": True,
        "js_runtimes": localizar_deno(),
    }
    if ffmpeg_dir:
        opcoes["ffmpeg_location"] = ffmpeg_dir
    return opcoes


# Segurança: impede que um nome de playlist como ".." faça o arquivo
# ser salvo fora da pasta de downloads.
PROTEGER_PASTA = {
    "key": "MetadataParser",
    "when": "pre_process",
    "actions": [(MetadataParserPP.replacer, "playlist_title", r"^\.+", "_")],
}


def opcoes_mp4(base: dict, altura_max: int) -> dict:
    return base | {
        "format": formato_mp4(altura_max),
        "postprocessors": [
            PROTEGER_PASTA,
            # Se o YouTube só tiver outro codec, converte para MP4 H.264 compatível.
            {"key": "FFmpegVideoConvertor", "preferedformat": "mp4"},
            {"key": "FFmpegMetadata", "add_metadata": True},
            {"key": "FFmpegThumbnailsConvertor", "format": "jpg", "when": "before_dl"},
            {"key": "EmbedThumbnail"},
        ],
    }


def opcoes_mp3(base: dict, qualidade_kbps: int) -> dict:
    return base | {
        "format": "ba[acodec^=mp4a]/ba/b",
        "postprocessors": [
            PROTEGER_PASTA,
            {"key": "FFmpegExtractAudio", "preferredcodec": "mp3",
             "preferredquality": str(qualidade_kbps)},
            {"key": "FFmpegMetadata", "add_metadata": True},
            {"key": "FFmpegThumbnailsConvertor", "format": "jpg", "when": "before_dl"},
            {"key": "EmbedThumbnail"},  # capa no MP3
        ],
    }


def baixar(links: list[str], opcoes: dict, rotulo: str) -> Resultado:
    resultado = Resultado()
    print(f"\n===== {rotulo}: {len(links)} link(s) =====")
    with yt_dlp.YoutubeDL(opcoes) as ydl:
        for i, link in enumerate(links, 1):
            print(f"\n[{rotulo} {i}/{len(links)}] {link}")
            try:
                codigo = ydl.download([link])
            except Exception as erro:  # noqa: BLE001
                print(f"  ERRO: {erro}")
                codigo = 1
            (resultado.ok if codigo == 0 else resultado.falhas).append(link)
    return resultado


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Baixa vídeos/playlists do YouTube em MP4 e MP3.")
    p.add_argument("links", nargs="*", help="links do YouTube (se vazio, lê o arquivo de links)")
    p.add_argument("-f", "--formato", choices=["ambos", "mp4", "mp3"], default="ambos",
                   help="o que baixar (padrão: ambos)")
    p.add_argument("-a", "--arquivo", type=Path, default=PASTA_DO_SCRIPT / "links.txt",
                   help="arquivo com um link por linha (padrão: links.txt)")
    p.add_argument("-d", "--destino", type=Path, default=PASTA_DO_SCRIPT / "downloads",
                   help="pasta de saída (padrão: downloads)")
    p.add_argument("-q", "--qualidade", type=int, default=720, choices=[360, 480, 720, 1080],
                   help="altura máxima do vídeo MP4 (padrão: 720)")
    p.add_argument("--kbps", type=int, default=192, choices=[128, 192, 256, 320],
                   help="qualidade do MP3 (padrão: 192)")
    p.add_argument("--playlist-inteira", action="store_true",
                   help="em links de vídeo que estão dentro de uma playlist "
                        "(watch?v=...&list=...), baixa a playlist toda")
    args = p.parse_args(argv)

    brutos = args.links or ler_links(args.arquivo)
    links, recusados = [], []
    for bruto in brutos:
        link = validar_link(bruto)
        if link and link not in links:
            links.append(link)
        elif not link:
            recusados.append(bruto)
    for r in recusados:
        print(f"Ignorado (não é link do YouTube): {r}")
    if not links:
        print("Nenhum link do YouTube para baixar. Coloque os links em links.txt, um por linha.")
        return 1

    playlists = sum(eh_playlist(l) for l in links)
    print(f"{len(links)} link(s) do YouTube ({playlists} playlist(s)).")

    destino = args.destino.resolve()
    destino.mkdir(parents=True, exist_ok=True)
    ffmpeg_dir = localizar_ffmpeg()

    resultados: dict[str, Resultado] = {}
    if args.formato in ("ambos", "mp4"):
        base = opcoes_base(destino / "MP4", destino / ".registro-mp4.txt", ffmpeg_dir,
                           args.playlist_inteira)
        resultados["MP4"] = baixar(links, opcoes_mp4(base, args.qualidade), "MP4")
    if args.formato in ("ambos", "mp3"):
        base = opcoes_base(destino / "MP3", destino / ".registro-mp3.txt", ffmpeg_dir,
                           args.playlist_inteira)
        resultados["MP3"] = baixar(links, opcoes_mp3(base, args.kbps), "MP3")

    print("\n===== RESUMO =====")
    falhas = []
    for rotulo, r in resultados.items():
        print(f"{rotulo}: {len(r.ok)} link(s) ok, {len(r.falhas)} com problema")
        falhas += [f"{rotulo}\t{l}" for l in r.falhas]
    arquivo_falhas = destino / "falhas.txt"
    if falhas:
        arquivo_falhas.write_text("\n".join(falhas) + "\n", encoding="utf-8")
        print(f"Links com problema salvos em: {arquivo_falhas}")
        print("Dica: rode de novo mais tarde — o que já foi baixado é pulado.")
    elif arquivo_falhas.exists():
        arquivo_falhas.unlink()
    print(f"Arquivos em: {destino}")
    return 1 if falhas else 0


if __name__ == "__main__":
    sys.exit(main())
