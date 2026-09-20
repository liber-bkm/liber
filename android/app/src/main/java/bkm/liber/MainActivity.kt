package bkm.liber

import android.app.Activity
import android.app.DownloadManager
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Environment
import android.util.Log
import android.view.KeyEvent
import android.webkit.URLUtil
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.TextView
import java.io.File
import java.net.HttpURLConnection
import java.net.ServerSocket
import java.net.URL
import java.net.URLEncoder

class MainActivity : Activity() {

    private var server: Process? = null
    private lateinit var web: WebView
    private var filePathCallback: ValueCallback<Array<Uri>>? = null
    private var pendingShare: String? = null
    private var serverPort: Int = -1

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        web = findViewById(R.id.web)
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val host = request.url.host ?: return false
                if (host == "127.0.0.1" || host == "localhost") return false
                startActivity(Intent(Intent.ACTION_VIEW, request.url))
                return true
            }
        }
        web.setDownloadListener { url, _, contentDisposition, _, _ ->
            try {
                val req = DownloadManager.Request(Uri.parse(url)).apply {
                    setNotificationVisibility(
                        DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED
                    )
                    setDestinationInExternalPublicDir(
                        Environment.DIRECTORY_DOWNLOADS,
                        "liber/" + URLUtil.guessFileName(url, contentDisposition, null)
                    )
                }
                (getSystemService(DOWNLOAD_SERVICE) as DownloadManager).enqueue(req)
            } catch (e: Exception) {
                Log.e(TAG, "download: $e")
                startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
            }
        }
        web.webChromeClient = object : WebChromeClient() {
            override fun onShowFileChooser(
                view: WebView,
                callback: ValueCallback<Array<Uri>>,
                params: FileChooserParams
            ): Boolean {
                filePathCallback?.onReceiveValue(null)
                filePathCallback = callback
                return try {
                    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                        addCategory(Intent.CATEGORY_OPENABLE)
                        type = "*/*"
                        putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                    }
                    startActivityForResult(Intent.createChooser(intent, "Select file"), FILE_CHOOSER_REQUEST)
                    true
                } catch (e: Exception) {
                    Log.e(TAG, "file chooser: $e")
                    filePathCallback?.onReceiveValue(null)
                    filePathCallback = null
                    false
                }
            }
        }
        val port = freePort()
        if (port <= 0 || !startServer(port)) {
            showFatal()
            return
        }
        pendingShare = sharedTarget(intent)
        waitReadyThenLoad(port)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        val shared = sharedTarget(intent) ?: return
        pendingShare = shared
        val port = serverPort
        if (port > 0 && ::web.isInitialized) {
            web.loadUrl(targetUrl(port))
        }
    }

    private fun sharedTarget(intent: Intent?): String? {
        if (intent?.action != Intent.ACTION_SEND) return null
        val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
            ?: intent.getStringExtra(Intent.EXTRA_SUBJECT)
            ?: return null
        extractUrl(text)?.let { return it }
        return text.trim().takeIf { it.isNotEmpty() }
    }

    private fun extractUrl(text: String): String? {
        val match = Regex("""https?://\S+""").find(text) ?: return null
        return match.value.trimEnd('.', ',', ')', '!', '?', '"', '\'').takeIf { it.isNotEmpty() }
    }

    private fun targetUrl(port: Int): String {
        val shared = pendingShare
        pendingShare = null
        if (shared == null) return "http://127.0.0.1:$port/"
        val encoded = URLEncoder.encode(shared, "UTF-8")
        return if (shared.startsWith("http")) {
            "http://127.0.0.1:$port/?prefill=$encoded"
        } else {
            "http://127.0.0.1:$port/?q=$encoded"
        }
    }

    private fun freePort(): Int {
        return try {
            ServerSocket(0).use { it.localPort }
        } catch (e: Exception) {
            Log.e(TAG, "no free port: $e")
            -1
        }
    }

    private fun startServer(port: Int): Boolean {
        return try {
            val lib = File(applicationInfo.nativeLibraryDir, "libliber.so")
            ProcessBuilder("chmod", "755", lib.absolutePath).start().waitFor()
            val pb = ProcessBuilder(
                lib.absolutePath, "--serve", "--addr", "127.0.0.1:$port"
            )
            pb.environment()["LIBER_BASE_DIR"] = File(filesDir, "bookmarks").absolutePath
            pb.environment()["LIBER_CONFIG"] = File(filesDir, "liber-config.json").absolutePath
            pb.redirectErrorStream(true)
            val proc = pb.start()
            server = proc
            Thread({
                try {
                    proc.inputStream.bufferedReader().forEachLine { Log.i(TAG, it) }
                } catch (_: Exception) {
                }
            }, "liber-log").start()
            true
        } catch (e: Exception) {
            Log.e(TAG, "start server: $e")
            false
        }
    }

    private fun waitReadyThenLoad(port: Int) {
        Thread({
            var ready = false
            for (i in 1..60) {
                try {
                    val c = URL("http://127.0.0.1:$port/").openConnection() as HttpURLConnection
                    c.connectTimeout = 500
                    c.readTimeout = 500
                    if (c.responseCode == 200) {
                        ready = true
                        break
                    }
                } catch (_: Exception) {
                }
                Thread.sleep(500)
            }
            runOnUiThread {
                if (ready) {
                    serverPort = port
                    web.loadUrl(targetUrl(port))
                } else {
                    showFatal()
                }
            }
        }, "liber-ready").start()
    }

    private fun showFatal() {
        val tv = TextView(this)
        tv.text = getString(R.string.server_failed)
        tv.setPadding(48, 48, 48, 48)
        setContentView(tv)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK && ::web.isInitialized && web.canGoBack()) {
            web.goBack()
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode != FILE_CHOOSER_REQUEST) {
            super.onActivityResult(requestCode, resultCode, data)
            return
        }
        val callback = filePathCallback
        filePathCallback = null
        if (callback == null) return
        if (resultCode != RESULT_OK || data == null) {
            callback.onReceiveValue(null)
            return
        }
        val uris = mutableListOf<Uri>()
        val clip = data.clipData
        if (clip != null) {
            for (i in 0 until clip.itemCount) {
                clip.getItemAt(i).uri?.let { uris.add(it) }
            }
        } else {
            data.data?.let { uris.add(it) }
        }
        for (uri in uris) {
            try {
                contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            } catch (_: Exception) {
            }
        }
        callback.onReceiveValue(if (uris.isEmpty()) null else uris.toTypedArray())
    }

    override fun onDestroy() {
        try {
            filePathCallback?.onReceiveValue(null)
        } catch (_: Exception) {
        }
        filePathCallback = null
        try {
            web.destroy()
        } catch (_: Exception) {
        }
        try {
            server?.destroy()
        } catch (_: Exception) {
        }
        server = null
        super.onDestroy()
    }

    companion object {
        private const val TAG = "LiberApp"
        private const val FILE_CHOOSER_REQUEST = 1001
    }
}
