<script lang="ts">
  import { onMount } from "svelte";
  import { motor, mensagemDeErro, extrairLink, tipoDeLink, type AudioFormat, type EngineStatus, type MediaInfo, type Kind } from "./lib/api";
  import { fila } from "./lib/fila.svelte";
  import { formatarDuracao } from "./lib/progresso";
  import ItemFila from "./lib/ItemFila.svelte";
  import Icone from "./lib/Icone.svelte";

  type Formato = Kind | "ambos";

  function lerPreferencia<T>(chave: string, padrao: T): T {
    try {
      const v = localStorage.getItem(`ytdownload.${chave}`);
      return v == null ? padrao : (JSON.parse(v) as T);
    } catch {
      return padrao;
    }
  }
  function salvarPreferencia(chave: string, valor: unknown) {
    try {
      localStorage.setItem(`ytdownload.${chave}`, JSON.stringify(valor));
    } catch {
      /* sem armazenamento */
    }
  }

  // ---------- Motor ----------
  let status = $state<EngineStatus | null>(null);
  let preparando = $state(true);
  let etapa = $state("Iniciando…");
  let etapaProgresso = $state<number | null>(null);
  let erroMotor = $state<string | null>(null);
  const celular = $derived(status?.platform === "android" || status?.platform === "ios");

  async function preparar() {
    preparando = true;
    erroMotor = null;
    try {
      status = await motor.prepare((e) => {
        if (e.type === "stage") {
          etapa = e.message;
          etapaProgresso = e.progress ?? null;
        }
      });
    } catch (e) {
      erroMotor = mensagemDeErro(e);
    } finally {
      preparando = false;
      etapaProgresso = null;
    }
  }

  // ---------- Entrada ----------
  let entrada = $state("");
  let formato = $state<Formato>(lerPreferencia("formato", "video"));
  let qualidade = $state<number>(lerPreferencia("qualidade", 720));
  let formatoAudio = $state<AudioFormat>(lerPreferencia("formatoAudio", "mp3"));
  let playlistInteira = $state(false);
  let cortar = $state(false);
  let inicio = $state("");
  let fim = $state("");

  let info = $state<MediaInfo | null>(null);
  let carregandoInfo = $state(false);
  let erroInfo = $state<string | null>(null);
  let verLista = $state(false);

  $effect(() => salvarPreferencia("formato", formato));
  $effect(() => salvarPreferencia("qualidade", qualidade));
  $effect(() => salvarPreferencia("formatoAudio", formatoAudio));
  const nomeAudio = $derived(formatoAudio.toUpperCase());

  const link = $derived(extrairLink(entrada));
  const tipo = $derived(link ? tipoDeLink(link) : null);
  const ehPlaylist = $derived(tipo === "playlist" || (tipo === "video-em-playlist" && playlistInteira));

  // Busca as informações do link automaticamente, com uma pequena espera.
  let consulta = 0;
  $effect(() => {
    const l = link;
    const p = ehPlaylist;
    info = null;
    erroInfo = null;
    verLista = false;
    if (!l || !status?.ready) return;
    const minha = ++consulta;
    carregandoInfo = true;
    const t = setTimeout(async () => {
      try {
        const r = await motor.inspect(l, p);
        if (minha === consulta) info = r;
      } catch (e) {
        if (minha === consulta) erroInfo = mensagemDeErro(e);
      } finally {
        if (minha === consulta) carregandoInfo = false;
      }
    }, 350);
    return () => clearTimeout(t);
  });

  async function colar() {
    try {
      // No Android a leitura é nativa (o WebView bloqueia navigator.clipboard).
      const nativo = celular ? (await motor.readClipboard()).url : null;
      const texto = nativo ?? (await navigator.clipboard.readText());
      if (texto) entrada = texto.trim();
    } catch {
      erroInfo = "Não consegui ler a área de transferência. Toque e segure no campo e escolha Colar.";
    }
  }

  function baixar() {
    if (!link) return;
    const tipos: Kind[] = formato === "ambos" ? ["video", "audio"] : [formato];
    const titulo = info?.title ?? link;
    for (const kind of tipos) {
      fila.adicionar({
        url: link,
        kind,
        quality: qualidade,
        playlist: ehPlaylist,
        audioFormat: formatoAudio,
        start: cortar && !ehPlaylist ? inicio.trim() || null : null,
        end: cortar && !ehPlaylist ? fim.trim() || null : null,
        titulo: ehPlaylist && info ? `${titulo} (${info.entries.length} itens)` : titulo,
        miniatura: info?.thumbnail ?? null,
      });
    }
    entrada = "";
    cortar = false;
    inicio = "";
    fim = "";
    playlistInteira = false;
  }

  // ---------- Links compartilhados (Android) ----------
  async function verCompartilhado() {
    try {
      const { url } = await motor.takeSharedUrl();
      if (url) entrada = url;
    } catch {
      /* desktop */
    }
  }

  // ---------- Configurações ----------
  let mostrarConfig = $state(false);
  let atualizando = $state(false);
  let avisoConfig = $state<string | null>(null);

  async function atualizar() {
    atualizando = true;
    avisoConfig = null;
    try {
      const v = await motor.update();
      avisoConfig = `Motor atualizado: yt-dlp ${v}`;
      status = await motor.status();
    } catch (e) {
      avisoConfig = mensagemDeErro(e);
    } finally {
      atualizando = false;
    }
  }

  async function trocarPasta() {
    try {
      status = await motor.chooseOutputDir();
    } catch (e) {
      avisoConfig = mensagemDeErro(e);
    }
  }

  async function abrirPasta() {
    try {
      await motor.openOutputDir();
    } catch (e) {
      avisoConfig = mensagemDeErro(e);
      mostrarConfig = true;
    }
  }

  onMount(() => {
    void preparar().then(verCompartilhado);
    const aoVoltar = () => document.visibilityState === "visible" && verCompartilhado();
    document.addEventListener("visibilitychange", aoVoltar);
    return () => document.removeEventListener("visibilitychange", aoVoltar);
  });

  const temConcluidas = $derived(fila.tarefas.some((t) => ["pronto", "erro", "cancelado"].includes(t.estado)));
</script>

<main>
  <header>
    <div class="marca">
      <span class="logo"><Icone nome="baixar" tamanho={18} /></span>
      <h1>YTDownload</h1>
    </div>
    <div class="cabecalho-acoes">
      <button class="icone" title="Abrir pasta dos downloads" aria-label="Abrir pasta" onclick={abrirPasta}><Icone nome="pasta" /></button>
      <button class="icone" title="Configurações" aria-label="Configurações" onclick={() => (mostrarConfig = !mostrarConfig)}><Icone nome="engrenagem" /></button>
    </div>
  </header>

  {#if mostrarConfig}
    <section class="cartao config">
      <h2>Configurações</h2>
      <dl>
        <dt>Pasta dos downloads</dt>
        <dd>
          <code>{status?.outputDir ?? "…"}</code>
          {#if !celular}<button class="link" onclick={trocarPasta}>Trocar</button>{/if}
        </dd>
        <dt>Motor (yt-dlp)</dt>
        <dd>
          {status?.version ?? "não instalado"}
          <button class="link" disabled={atualizando} onclick={atualizar}>{atualizando ? "Atualizando…" : "Atualizar agora"}</button>
        </dd>
      </dl>
      <p class="nota">O motor se atualiza sozinho a cada 2 dias. Se um vídeo parar de baixar de repente, toque em “Atualizar agora”.</p>
      {#if avisoConfig}<p class="nota destaque">{avisoConfig}</p>{/if}
    </section>
  {/if}

  {#if preparando && !status?.ready}
    <section class="cartao preparo">
      <p><strong>{etapa}</strong></p>
      <div class="barra" class:indeterminada={etapaProgresso == null}>
        <div style:width={etapaProgresso == null ? "35%" : `${Math.round(etapaProgresso * 100)}%`}></div>
      </div>
      <p class="nota">Na primeira vez o app baixa o motor de download. Isso só acontece uma vez.</p>
    </section>
  {:else if erroMotor && !status?.ready}
    <section class="cartao erro-motor">
      <p><strong>Não consegui preparar o motor.</strong></p>
      <p class="nota">{erroMotor}</p>
      <button class="primario" onclick={preparar}>Tentar de novo</button>
    </section>
  {:else}
    <section class="cartao entrada">
      <label for="link" class="rotulo">Link do YouTube</label>
      <div class="campo">
        <input
          id="link"
          type="url"
          inputmode="url"
          autocomplete="off"
          spellcheck="false"
          placeholder="Cole o link do vídeo ou playlist"
          bind:value={entrada}
          onkeydown={(e) => e.key === "Enter" && link && baixar()}
        />
        {#if entrada}
          <button class="icone" aria-label="Limpar" onclick={() => (entrada = "")}><Icone nome="x" /></button>
        {:else}
          <button class="botao-colar" onclick={colar}><Icone nome="colar" tamanho={18} /> Colar</button>
        {/if}
      </div>
      {#if entrada && !link}<p class="nota msg-erro">Isso não parece um link do YouTube.</p>{/if}

      {#if link}
        <div class="previa" class:carregando={carregandoInfo}>
          {#if info?.thumbnail}<img src={info.thumbnail} alt="" referrerpolicy="no-referrer" />{/if}
          <div>
            {#if carregandoInfo}
              <p class="nota">Buscando informações…</p>
            {:else if info}
              <p class="titulo">{info.title}</p>
              <p class="nota">
                {info.uploader ?? ""}
                {#if info.isPlaylist} · {info.entries.length} itens{:else if info.duration} · {formatarDuracao(info.duration)}{/if}
              </p>
              {#if info.isPlaylist && info.entries.length}
                <button class="link" onclick={() => (verLista = !verLista)}><Icone nome="lista" tamanho={16} /> {verLista ? "Esconder" : "Ver"} lista</button>
              {/if}
            {:else if erroInfo}
              <p class="nota msg-erro">{erroInfo}</p>
            {/if}
          </div>
        </div>
        {#if verLista && info}
          <ol class="lista">
            {#each info.entries as e (e.id)}
              <li><span>{e.title}</span><span class="nota">{formatarDuracao(e.duration)}</span></li>
            {/each}
          </ol>
        {/if}
      {/if}

      <div class="opcoes">
        <div class="segmentado" role="radiogroup" aria-label="Formato">
          <button role="radio" aria-checked={formato === "video"} class:ativo={formato === "video"} onclick={() => (formato = "video")}><Icone nome="video" tamanho={18} /> Vídeo</button>
          <button role="radio" aria-checked={formato === "audio"} class:ativo={formato === "audio"} onclick={() => (formato = "audio")}><Icone nome="musica" tamanho={18} /> Áudio</button>
          <button role="radio" aria-checked={formato === "ambos"} class:ativo={formato === "ambos"} onclick={() => (formato = "ambos")}>Os dois</button>
        </div>

        {#if formato !== "video"}
          <label class="select">
            Áudio
            <select bind:value={formatoAudio}>
              <option value="mp3">MP3 (toca em tudo)</option>
              <option value="m4a">M4A (bem mais rápido)</option>
            </select>
          </label>
        {/if}
        {#if formato !== "audio"}
          <label class="select">
            Qualidade
            <select bind:value={qualidade}>
              <option value={360}>360p (menor)</option>
              <option value={480}>480p</option>
              <option value={720}>720p (HD)</option>
              <option value={1080}>1080p (Full HD)</option>
            </select>
          </label>
        {/if}
      </div>

      {#if tipo === "video-em-playlist"}
        <label class="checkbox"><input type="checkbox" bind:checked={playlistInteira} /> Baixar a playlist inteira</label>
      {/if}

      {#if !ehPlaylist}
        <label class="checkbox"><input type="checkbox" bind:checked={cortar} /> <Icone nome="tesoura" tamanho={16} /> Baixar só um trecho</label>
        {#if cortar}
          <div class="trecho">
            <label>De <input placeholder="0:00" inputmode="numeric" bind:value={inicio} /></label>
            <label>até <input placeholder="fim" inputmode="numeric" bind:value={fim} /></label>
          </div>
        {/if}
      {/if}

      <button class="primario grande" disabled={!link} onclick={baixar}>
        <Icone nome="baixar" /> Baixar {formato === "ambos" ? `MP4 + ${nomeAudio}` : formato === "video" ? "MP4" : nomeAudio}
      </button>
      <p class="nota centro">
        {#if formato !== "video" && formatoAudio === "m4a"}
          M4A sai quase tão rápido quanto o vídeo, mas som de carro e caixinhas antigas podem só aceitar MP3.
        {:else}
          MP4 funciona em celular, TV e computador. MP3 é só o áudio, para som do carro e caixinhas.
        {/if}
      </p>
    </section>
  {/if}

  {#if fila.tarefas.length}
    <section class="downloads">
      <div class="titulo-secao">
        <h2>Downloads</h2>
        {#if temConcluidas}<button class="link" onclick={() => fila.limparConcluidas()}>Limpar concluídos</button>{/if}
      </div>
      <ul>
        {#each fila.tarefas as tarefa (tarefa.id)}
          <ItemFila {tarefa} {celular} />
        {/each}
      </ul>
    </section>
  {/if}
</main>
