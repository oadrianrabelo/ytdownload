package com.adrianrabelo.ytdlp

import android.app.Activity
import android.app.DownloadManager
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.view.WindowManager
import android.webkit.MimeTypeMap
import android.webkit.WebView
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.yausername.ffmpeg.FFmpeg
import com.yausername.youtubedl_android.YoutubeDL
import com.yausername.youtubedl_android.YoutubeDLRequest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import java.io.File
import java.util.concurrent.atomic.AtomicInteger

@InvokeArg
class PrepareArgs {
    var channel: Channel? = null
}

@InvokeArg
class RunArgs {
    var id: String? = null
    var args: List<String> = emptyList()
    var folder: String? = null
    var channel: Channel? = null
}

@InvokeArg
class IdArgs {
    var id: String = ""
}

@InvokeArg
class PathArgs {
    var path: String = ""
}

/** Pasta pública onde os arquivos aparecem para o usuário: Download/YTDownload. */
private const val PASTA_PUBLICA = "YTDownload"
private const val DOIS_DIAS_MS = 2L * 24 * 3600 * 1000
private val LINK = Regex("""https?://\S+""")

@TauriPlugin
class YtdlpPlugin(private val activity: Activity) : Plugin(activity) {
    private val escopo = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val ativos = AtomicInteger(0)
    private val contexto: Context get() = activity.applicationContext

    @Volatile
    private var linkCompartilhado: String? = null

    // ---------- Receber links pelo "Compartilhar" do YouTube ----------

    override fun load(webView: WebView) {
        super.load(webView)
        guardarLink(activity.intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        guardarLink(intent)
    }

    private fun guardarLink(intent: Intent?) {
        if (intent?.action != Intent.ACTION_SEND) return
        val texto = intent.getStringExtra(Intent.EXTRA_TEXT) ?: return
        LINK.find(texto)?.value?.let { linkCompartilhado = it }
    }

    @Command
    fun takeSharedUrl(invoke: Invoke) {
        val r = JSObject()
        r.put("url", linkCompartilhado)
        linkCompartilhado = null
        invoke.resolve(r)
    }

    // ---------- Motor ----------

    private fun iniciar() {
        YoutubeDL.getInstance().init(contexto)
        FFmpeg.getInstance().init(contexto)
    }

    private fun prefs() = contexto.getSharedPreferences("ytdownload", Context.MODE_PRIVATE)

    private fun status(pronto: Boolean): JSObject {
        val r = JSObject()
        r.put("ready", pronto)
        r.put("version", YoutubeDL.getInstance().versionName(contexto) ?: YoutubeDL.getInstance().version(contexto))
        r.put("outputDir", "${Environment.DIRECTORY_DOWNLOADS}/$PASTA_PUBLICA")
        r.put("platform", "android")
        return r
    }

    private fun etapa(canal: Channel?, mensagem: String) {
        val e = JSObject()
        e.put("type", "stage")
        e.put("message", mensagem)
        canal?.send(e)
    }

    @Command
    fun status(invoke: Invoke) {
        escopo.launch {
            val pronto = try { iniciar(); true } catch (e: Exception) { false }
            invoke.resolve(status(pronto))
        }
    }

    @Command
    fun prepare(invoke: Invoke) {
        val args = invoke.parseArgs(PrepareArgs::class.java)
        escopo.launch {
            try {
                etapa(args.channel, "Preparando o motor…")
                iniciar()
                val ultima = prefs().getLong("ultimaAtualizacao", 0)
                if (System.currentTimeMillis() - ultima > DOIS_DIAS_MS) {
                    etapa(args.channel, "Procurando atualização do yt-dlp…")
                    try {
                        YoutubeDL.getInstance().updateYoutubeDL(contexto, YoutubeDL.UpdateChannel.STABLE)
                        prefs().edit().putLong("ultimaAtualizacao", System.currentTimeMillis()).apply()
                    } catch (e: Exception) {
                        // Sem internet: segue com a versão atual.
                    }
                }
                invoke.resolve(status(true))
            } catch (e: Exception) {
                invoke.reject("Falha ao preparar o motor: ${e.message}")
            }
        }
    }

    @Command
    fun update(invoke: Invoke) {
        escopo.launch {
            try {
                iniciar()
                YoutubeDL.getInstance().updateYoutubeDL(contexto, YoutubeDL.UpdateChannel.STABLE)
                prefs().edit().putLong("ultimaAtualizacao", System.currentTimeMillis()).apply()
                val r = JSObject()
                r.put("version", YoutubeDL.getInstance().versionName(contexto) ?: "")
                invoke.resolve(r)
            } catch (e: Exception) {
                invoke.reject("Não consegui atualizar: ${e.message}")
            }
        }
    }

    /**
     * Roda o yt-dlp com argumentos montados (e validados) pelo lado Rust.
     * Com `folder`, baixa numa pasta temporária e depois move para Download/YTDownload/<folder>.
     */
    @Command
    fun run(invoke: Invoke) {
        val args = invoke.parseArgs(RunArgs::class.java)
        escopo.launch {
            val temp = args.folder?.let { File(contexto.cacheDir, "downloads/${args.id ?: System.nanoTime()}") }
            val r = JSObject()
            var cancelado = false
            if (temp != null) manterTelaLigada(+1)
            try {
                iniciar()
                temp?.deleteRecursively()
                temp?.mkdirs()
                val comandos = mutableListOf<String>()
                if (temp != null) comandos += listOf("-P", temp.absolutePath)
                comandos += args.args
                val pedido = YoutubeDLRequest(emptyList<String>()).addCommands(comandos)
                val canal = args.channel
                val resposta = YoutubeDL.getInstance().execute(pedido, args.id) { _, _, linha ->
                    if (canal != null) {
                        val e = JSObject()
                        e.put("type", "line")
                        e.put("line", linha)
                        canal.send(e)
                    }
                }
                r.put("exitCode", resposta.exitCode)
                r.put("out", resposta.out)
                r.put("err", resposta.err)
            } catch (e: YoutubeDL.CanceledException) {
                cancelado = true
                r.put("exitCode", 1).put("out", "").put("err", "")
            } catch (e: Exception) {
                // Erro do yt-dlp: a mensagem contém a saída de erro.
                r.put("exitCode", 1).put("out", "").put("err", e.message ?: e.toString())
            }
            val arquivos = JSArray()
            if (temp != null) {
                if (!cancelado) {
                    try {
                        publicar(temp, args.folder!!).forEach { arquivos.put(it) }
                    } catch (e: Exception) {
                        r.put("err", "ERROR: Não consegui salvar na pasta Download: ${e.message}")
                    }
                }
                temp.deleteRecursively()
                manterTelaLigada(-1)
            }
            r.put("files", arquivos)
            r.put("canceled", cancelado)
            invoke.resolve(r)
        }
    }

    @Command
    fun cancel(invoke: Invoke) {
        val args = invoke.parseArgs(IdArgs::class.java)
        YoutubeDL.getInstance().destroyProcessById(args.id)
        invoke.resolve()
    }

    /** Evita que a tela apague (e o Android pause o app) durante downloads. */
    private fun manterTelaLigada(delta: Int) {
        val n = ativos.addAndGet(delta)
        activity.runOnUiThread {
            if (n > 0) activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            else activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
    }

    // ---------- Salvar na pasta pública ----------

    private fun tipo(arquivo: File): String =
        MimeTypeMap.getSingleton().getMimeTypeFromExtension(arquivo.extension.lowercase())
            ?: when (arquivo.extension.lowercase()) {
                "mp3" -> "audio/mpeg"
                "mp4" -> "video/mp4"
                else -> "application/octet-stream"
            }

    private fun publicar(temp: File, pasta: String): List<JSObject> {
        val ignorar = setOf("part", "ytdl", "temp", "jpg", "webp", "png")
        return temp.walkTopDown()
            .filter { it.isFile && it.extension.lowercase() !in ignorar }
            .map { arquivo ->
                val sub = arquivo.parentFile!!.relativeTo(temp).path
                val relativo = listOf(PASTA_PUBLICA, pasta, sub).filter { it.isNotEmpty() }.joinToString("/")
                val uri = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    salvarMediaStore(arquivo, relativo)
                } else {
                    salvarLegado(arquivo, relativo)
                }
                val o = JSObject()
                o.put("name", arquivo.name)
                o.put("path", uri.toString())
                o
            }
            .toList()
    }

    private fun salvarMediaStore(arquivo: File, relativo: String): Uri {
        val resolver = contexto.contentResolver
        val valores = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, arquivo.name)
            put(MediaStore.MediaColumns.MIME_TYPE, tipo(arquivo))
            put(MediaStore.MediaColumns.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/$relativo")
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        val colecao = MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
        val uri = resolver.insert(colecao, valores) ?: error("MediaStore recusou o arquivo")
        try {
            resolver.openOutputStream(uri)!!.use { saida -> arquivo.inputStream().use { it.copyTo(saida) } }
            valores.clear()
            valores.put(MediaStore.MediaColumns.IS_PENDING, 0)
            resolver.update(uri, valores, null, null)
        } catch (e: Exception) {
            resolver.delete(uri, null, null)
            throw e
        }
        return uri
    }

    /** Android 9 ou anterior: salva na pasta do app e compartilha por FileProvider. */
    private fun salvarLegado(arquivo: File, relativo: String): Uri {
        val base = contexto.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS) ?: contexto.filesDir
        val destino = File(base, "$relativo/${arquivo.name}")
        destino.parentFile?.mkdirs()
        arquivo.copyTo(destino, overwrite = true)
        return FileProvider.getUriForFile(contexto, "${contexto.packageName}.fileprovider", destino)
    }

    // ---------- Abrir e compartilhar ----------

    private fun uriPermitida(texto: String): Uri {
        val uri = Uri.parse(texto)
        require(uri.scheme == "content") { "Arquivo inválido" }
        return uri
    }

    @Command
    fun openFile(invoke: Invoke) {
        try {
            val uri = uriPermitida(invoke.parseArgs(PathArgs::class.java).path)
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, contexto.contentResolver.getType(uri))
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(Intent.createChooser(intent, "Abrir com"))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Não consegui abrir: ${e.message}")
        }
    }

    @Command
    fun shareFile(invoke: Invoke) {
        try {
            val uri = uriPermitida(invoke.parseArgs(PathArgs::class.java).path)
            val intent = Intent(Intent.ACTION_SEND).apply {
                type = contexto.contentResolver.getType(uri) ?: "*/*"
                putExtra(Intent.EXTRA_STREAM, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(Intent.createChooser(intent, "Enviar para"))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Não consegui compartilhar: ${e.message}")
        }
    }

    @Command
    fun openOutputDir(invoke: Invoke) {
        try {
            activity.startActivity(Intent(DownloadManager.ACTION_VIEW_DOWNLOADS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Abra o app Arquivos e procure a pasta Download/$PASTA_PUBLICA.")
        }
    }
}
