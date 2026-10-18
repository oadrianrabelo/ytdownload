// Fila de downloads: no máximo 2 ao mesmo tempo, o resto espera.
import { motor, mensagemDeErro, type AudioFormat, type Kind, type SavedFile } from "./api";
import { lerLinha } from "./progresso";

export type Estado = "fila" | "baixando" | "convertendo" | "pronto" | "erro" | "cancelado";

export interface Tarefa {
  id: string;
  url: string;
  kind: Kind;
  quality: number;
  playlist: boolean;
  /** Ausente em downloads salvos por versões antigas (= mp3). */
  audioFormat?: AudioFormat;
  start: string | null;
  end: string | null;
  titulo: string;
  miniatura: string | null;
  estado: Estado;
  fracao: number | null;
  velocidade: number | null;
  eta: number | null;
  item: { indice: number; total: number } | null;
  arquivos: SavedFile[];
  erro: string | null;
  criada: number;
}

const SIMULTANEOS = 2;
const CHAVE = "ytdownload.historico";

function carregar(): Tarefa[] {
  try {
    const salvas: Tarefa[] = JSON.parse(localStorage.getItem(CHAVE) ?? "[]");
    // Downloads interrompidos quando o app fechou voltam como erro, para tentar de novo.
    return salvas.map((t) =>
      ["fila", "baixando", "convertendo"].includes(t.estado)
        ? { ...t, estado: "erro" as Estado, erro: "Interrompido quando o app foi fechado." }
        : t,
    );
  } catch {
    return [];
  }
}

class Fila {
  tarefas = $state<Tarefa[]>(carregar());

  ativas = $derived(this.tarefas.filter((t) => t.estado === "baixando" || t.estado === "convertendo").length);

  private salvar() {
    try {
      const finais = this.tarefas.slice(0, 100);
      localStorage.setItem(CHAVE, JSON.stringify(finais));
    } catch {
      /* armazenamento indisponível: segue sem histórico */
    }
  }

  adicionar(t: Omit<Tarefa, "id" | "estado" | "fracao" | "velocidade" | "eta" | "item" | "arquivos" | "erro" | "criada">) {
    this.tarefas.unshift({
      ...t,
      id: crypto.randomUUID(),
      estado: "fila",
      fracao: null,
      velocidade: null,
      eta: null,
      item: null,
      arquivos: [],
      erro: null,
      criada: Date.now(),
    });
    this.salvar();
    this.proxima();
  }

  private proxima() {
    while (this.ativas < SIMULTANEOS) {
      const t = [...this.tarefas].reverse().find((x) => x.estado === "fila");
      if (!t) return;
      t.estado = "baixando";
      void this.executar(t);
    }
  }

  private async executar(t: Tarefa) {
    // Acessa sempre pelo proxy reativo, para a tela atualizar.
    const tarefa = this.tarefas.find((x) => x.id === t.id)!;
    try {
      const r = await motor.download(
        {
          id: tarefa.id,
          url: tarefa.url,
          kind: tarefa.kind,
          quality: tarefa.quality,
          playlist: tarefa.playlist,
          audioFormat: tarefa.audioFormat ?? "mp3",
          start: tarefa.start,
          end: tarefa.end,
        },
        (e) => {
          if (e.type !== "line") return;
          const l = lerLinha(e.line);
          if (!l) return;
          if (l.tipo === "progresso") {
            tarefa.estado = "baixando";
            tarefa.fracao = l.valor.fracao;
            tarefa.velocidade = l.valor.velocidade;
            tarefa.eta = l.valor.eta;
          } else if (l.tipo === "item") {
            if (l.indice && l.total) tarefa.item = { indice: l.indice, total: l.total };
            if (!tarefa.playlist && l.titulo) tarefa.titulo = l.titulo;
            tarefa.fracao = 0;
          } else if (l.tipo === "convertendo") {
            tarefa.estado = "convertendo";
          }
        },
      );
      tarefa.arquivos = r.files;
      tarefa.erro = r.error;
      tarefa.estado = r.files.length || !r.error ? "pronto" : "erro";
      if (r.files.length === 0 && !r.error) tarefa.erro = "Já estava baixado.";
    } catch (e) {
      const msg = mensagemDeErro(e);
      tarefa.estado = msg.startsWith("Cancelado") ? "cancelado" : "erro";
      tarefa.erro = tarefa.estado === "erro" ? msg : null;
    } finally {
      tarefa.velocidade = null;
      tarefa.eta = null;
      this.salvar();
      this.proxima();
    }
  }

  cancelar(id: string) {
    const t = this.tarefas.find((x) => x.id === id);
    if (!t) return;
    if (t.estado === "fila") {
      t.estado = "cancelado";
      this.salvar();
    } else {
      void motor.cancel(id);
    }
  }

  repetir(id: string) {
    const t = this.tarefas.find((x) => x.id === id);
    if (!t) return;
    Object.assign(t, { estado: "fila", erro: null, fracao: null, item: null, arquivos: [] });
    this.salvar();
    this.proxima();
  }

  remover(id: string) {
    this.tarefas = this.tarefas.filter((x) => x.id !== id);
    this.salvar();
  }

  limparConcluidas() {
    this.tarefas = this.tarefas.filter((x) => !["pronto", "cancelado", "erro"].includes(x.estado));
    this.salvar();
  }
}

export const fila = new Fila();
