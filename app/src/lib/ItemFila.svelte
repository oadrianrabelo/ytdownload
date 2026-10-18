<script lang="ts">
  import { motor, mensagemDeErro } from "./api";
  import { fila, type Tarefa } from "./fila.svelte";
  import { formatarBytes, formatarDuracao } from "./progresso";
  import Icone from "./Icone.svelte";

  let { tarefa, celular }: { tarefa: Tarefa; celular: boolean } = $props();
  let aviso = $state<string | null>(null);

  const rotulo: Record<string, string> = {
    fila: "Na fila",
    baixando: "Baixando",
    convertendo: "Finalizando…",
    pronto: "Pronto",
    erro: "Erro",
    cancelado: "Cancelado",
  };

  const emAndamento = $derived(tarefa.estado === "baixando" || tarefa.estado === "convertendo");
  const porcentagem = $derived(tarefa.fracao == null ? null : Math.round(tarefa.fracao * 100));

  async function acao(f: () => Promise<void>) {
    aviso = null;
    try {
      await f();
    } catch (e) {
      aviso = mensagemDeErro(e);
    }
  }
</script>

<li class="item" class:pronto={tarefa.estado === "pronto"} class:erro={tarefa.estado === "erro"}>
  {#if tarefa.miniatura}
    <img class="mini" src={tarefa.miniatura} alt="" loading="lazy" referrerpolicy="no-referrer" />
  {:else}
    <div class="mini vazio"><Icone nome={tarefa.kind === "video" ? "video" : "musica"} /></div>
  {/if}

  <div class="corpo">
    <div class="topo">
      <span class="tipo" class:audio={tarefa.kind === "audio"}>{tarefa.kind === "video" ? `MP4 ${tarefa.quality}p` : "MP3"}</span>
      {#if tarefa.start || tarefa.end}<span class="tipo trecho">{tarefa.start || "0:00"}–{tarefa.end || "fim"}</span>{/if}
      <span class="estado">{rotulo[tarefa.estado]}</span>
    </div>
    <p class="titulo" title={tarefa.titulo}>{tarefa.titulo}</p>

    {#if emAndamento || tarefa.estado === "fila"}
      <div class="barra" class:indeterminada={porcentagem == null || tarefa.estado === "convertendo"}>
        <div style:width={porcentagem == null ? "35%" : `${porcentagem}%`}></div>
      </div>
      <p class="detalhe">
        {#if tarefa.item}Item {tarefa.item.indice} de {tarefa.item.total} · {/if}
        {#if tarefa.estado === "baixando" && porcentagem != null}{porcentagem}%{/if}
        {#if tarefa.velocidade} · {formatarBytes(tarefa.velocidade)}{/if}
        {#if tarefa.eta} · faltam {formatarDuracao(tarefa.eta)}{/if}
      </p>
    {:else if tarefa.erro}
      <p class="detalhe msg-erro">{tarefa.erro}</p>
    {:else if tarefa.arquivos.length > 1}
      <p class="detalhe">{tarefa.arquivos.length} arquivos salvos</p>
    {/if}
    {#if aviso}<p class="detalhe msg-erro">{aviso}</p>{/if}
  </div>

  <div class="acoes">
    {#if emAndamento || tarefa.estado === "fila"}
      <button class="icone" title="Cancelar" aria-label="Cancelar" onclick={() => fila.cancelar(tarefa.id)}><Icone nome="x" /></button>
    {:else}
      {#if tarefa.estado === "pronto" && tarefa.arquivos.length}
        {@const arquivo = tarefa.arquivos[0]}
        {#if tarefa.arquivos.length === 1}
          <button class="icone" title="Abrir" aria-label="Abrir" onclick={() => acao(() => motor.openFile(arquivo.path))}><Icone nome="play" /></button>
        {/if}
        {#if celular}
          <button class="icone" title="Enviar (WhatsApp, Drive…)" aria-label="Compartilhar" onclick={() => acao(() => motor.shareFile(arquivo.path))}><Icone nome="compartilhar" /></button>
        {:else}
          <button class="icone" title="Mostrar na pasta" aria-label="Mostrar na pasta" onclick={() => acao(() => motor.revealFile(arquivo.path))}><Icone nome="pasta" /></button>
        {/if}
      {/if}
      {#if tarefa.estado === "erro" || tarefa.estado === "cancelado"}
        <button class="icone" title="Tentar de novo" aria-label="Tentar de novo" onclick={() => fila.repetir(tarefa.id)}><Icone nome="repetir" /></button>
      {/if}
      <button class="icone discreto" title="Tirar da lista" aria-label="Tirar da lista" onclick={() => fila.remover(tarefa.id)}><Icone nome="lixo" tamanho={18} /></button>
    {/if}
  </div>
</li>
