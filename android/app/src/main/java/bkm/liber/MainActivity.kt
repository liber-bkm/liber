package bkm.liber

import android.app.Activity
import android.app.AlertDialog
import android.app.DownloadManager
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.text.InputType
import android.util.Log
import android.view.KeyEvent
import android.view.Menu
import android.webkit.CookieManager
import android.webkit.URLUtil
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.PopupMenu
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.TextView
import android.widget.Toast
import bkm.liber.ui.UiActivity
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
    private var activeBase: String = ""
    private var internalHost: String = ""
    private var lastUrl: String = ""
    private var loadAttempts: Int = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        web = findViewById(R.id.web)
        findViewById<Button>(R.id.menu).setOnClickListener { anchor -> showOverflowMenu(anchor) }
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val host = request.url.host ?: return false
                if (host == "127.0.0.1" || host == "localhost" || host == internalHost) return false
                startActivity(Intent(Intent.ACTION_VIEW, request.url))
                return true
            }
            override fun onReceivedError(
                view: WebView,
                request: WebResourceRequest,
                error: WebResourceError
            ) {
                if (!request.isForMainFrame || lastUrl.isEmpty()) return
                if (loadAttempts < MAX_LOAD_ATTEMPTS) {
                    loadAttempts++
                    view.postDelayed({ view.loadUrl(lastUrl) }, LOAD_RETRY_DELAY_MS)
                } else {
                    showFatalRetry()
                }
            }
            override fun onPageFinished(view: WebView, url: String) {
                loadAttempts = 0
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
        routeStartup()
    }

    private fun prefs() = getPreferences(MODE_PRIVATE)

    private fun routeStartup() {
        if (prefs().getString(Prefs.MODE, null) == null) {
            showModeDialog(firstRun = true)
            return
        }
        applyMode()
    }

    private fun applyMode() {
        if (prefs().getString(Prefs.MODE, Prefs.STANDALONE) == Prefs.REMOTE) {
            val base = normalizeBase(prefs().getString(Prefs.SERVER_URL, ""))
            if (base == null) {
                showModeDialog(firstRun = false)
                return
            }
            stopServer()
            activeBase = base
            internalHost = Uri.parse(base).host ?: ""
            pendingShare = sharedTarget(intent)
            loginRemoteThenLoad(base)
        } else {
            startStandalone()
        }
    }

    private fun startStandalone() {
        stopServer()
        activeBase = ""
        internalHost = ""
        val port = freePort()
        if (port <= 0 || !startServer(port)) {
            showFatal()
            return
        }
        val base = "http://127.0.0.1:$port"
        activeBase = base
        internalHost = "127.0.0.1"
        pendingShare = sharedTarget(intent)
        loadAppUrl(targetUrl(base))
    }

    private fun stopServer() {
        try {
            server?.destroy()
        } catch (_: Exception) {
        }
        server = null
    }

    private fun normalizeBase(raw: String?): String? {
        var base = raw?.trim()?.trimEnd('/') ?: return null
        if (base.isEmpty()) return null
        if (!base.contains("://")) base = "http://$base"
        return base
    }

    private fun showModeDialog(firstRun: Boolean) {
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 32, 48, 0)
        }
        val group = RadioGroup(this).apply { orientation = RadioGroup.VERTICAL }
        val standalone = RadioButton(this).apply { text = "Standalone (on this device)" }
        val remote = RadioButton(this).apply { text = "Connect to a server" }
        group.addView(standalone)
        group.addView(remote)
        val urlField = EditText(this).apply {
            hint = "Server URL, e.g. http://192.168.1.10:8080"
            setText(prefs().getString(Prefs.SERVER_URL, ""))
        }
        val tokenField = EditText(this).apply {
            hint = "Auth token (only if the server requires one)"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            setText(prefs().getString(Prefs.SERVER_TOKEN, ""))
        }
        layout.addView(group)
        layout.addView(urlField)
        layout.addView(tokenField)
        if (prefs().getString(Prefs.MODE, Prefs.STANDALONE) == Prefs.REMOTE) {
            remote.isChecked = true
        } else {
            standalone.isChecked = true
        }
        val dialog = AlertDialog.Builder(this)
            .setTitle("How should liber run?")
            .setMessage("Standalone keeps bookmarks on this device. Server mode connects to your desktop over a URL you trust (tunnel or local network only).")
            .setView(layout)
            .setCancelable(!firstRun)
            .setPositiveButton("Save", null)
            .create()
        if (!firstRun) {
            dialog.setButton(AlertDialog.BUTTON_NEGATIVE, "Cancel") { _, _ -> }
        }
        dialog.show()
        dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener {
            val wantRemote = remote.isChecked
            val url = urlField.text.toString()
            if (wantRemote && normalizeBase(url) == null) {
                toast("Enter a server URL.")
                return@setOnClickListener
            }
            prefs().edit()
                .putString(Prefs.MODE, if (wantRemote) Prefs.REMOTE else Prefs.STANDALONE)
                .putString(Prefs.SERVER_URL, url.trim())
                .putString(Prefs.SERVER_TOKEN, tokenField.text.toString().trim())
                .apply()
            dialog.dismiss()
            applyMode()
        }
    }

    private fun loginRemoteThenLoad(base: String) {
        val token = prefs().getString(Prefs.SERVER_TOKEN, "") ?: ""
        if (token.isEmpty()) {
            loadAppUrl(targetUrl(base))
            return
        }
        Thread({
            var cookies: List<String>? = null
            try {
                val c = URL(base + "/login").openConnection() as HttpURLConnection
                c.requestMethod = "POST"
                c.doOutput = true
                c.connectTimeout = 8000
                c.readTimeout = 8000
                c.instanceFollowRedirects = false
                c.setRequestProperty("Content-Type", "application/x-www-form-urlencoded")
                val body = "token=" + URLEncoder.encode(token, "UTF-8") +
                    "&next=" + URLEncoder.encode("/", "UTF-8")
                c.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
                if (c.responseCode == HttpURLConnection.HTTP_SEE_OTHER) {
                    cookies = c.headerFields["Set-Cookie"] ?: emptyList()
                } else {
                    Log.e(TAG, "remote login rejected (HTTP ${c.responseCode})")
                }
            } catch (e: Exception) {
                Log.e(TAG, "remote login: $e")
            }
            runOnUiThread {
                if (cookies != null) {
                    val cm = CookieManager.getInstance()
                    cm.setAcceptCookie(true)
                    for (h in cookies!!) {
                        cm.setCookie(base, h)
                    }
                    cm.flush()
                }
                loadAppUrl(targetUrl(base))
            }
        }, "liber-login").start()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        val shared = sharedTarget(intent) ?: return
        pendingShare = shared
        val base = activeBase
        if (base.isNotEmpty() && ::web.isInitialized) {
            loadAppUrl(targetUrl(base))
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

    private fun loadAppUrl(url: String) {
        lastUrl = url
        loadAttempts = 0
        web.loadUrl(url)
    }

    private fun retryLoad() {
        loadAttempts = 0
        val proc = server
        val remote = prefs().getString(Prefs.MODE, Prefs.STANDALONE) == Prefs.REMOTE
        if (lastUrl.isNotEmpty() && (remote || (proc != null && proc.isAlive))) {
            web.loadUrl(lastUrl)
        } else {
            routeStartup()
        }
    }
    private fun targetUrl(base: String): String {
        val shared = pendingShare
        pendingShare = null
        if (shared == null) return "$base/"
        val encoded = URLEncoder.encode(shared, "UTF-8")
        return if (shared.startsWith("http")) {
            "$base/?prefill=$encoded"
        } else {
            "$base/?q=$encoded"
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

    private fun showFatal() {
        val tv = TextView(this)
        tv.text = getString(R.string.server_failed)
        tv.setPadding(48, 48, 48, 48)
        setContentView(tv)
    }

    private fun showFatalRetry() {
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
        }
        val tv = TextView(this).apply {
            text = getString(R.string.server_failed)
        }
        val retry = Button(this).apply {
            text = "Retry"
            setOnClickListener { retryLoad() }
        }
        layout.addView(tv)
        layout.addView(retry)
        setContentView(layout)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK && ::web.isInitialized && web.canGoBack()) {
            web.goBack()
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    private fun showOverflowMenu(anchor: android.view.View) {
        val popup = PopupMenu(this, anchor)
        popup.menu.add(Menu.NONE, MENU_SYNC_FOLDER, Menu.NONE, "Sync folder")
        popup.menu.add(Menu.NONE, MENU_EXPORT_SYNC, Menu.NONE, "Export to sync folder")
        popup.menu.add(Menu.NONE, MENU_SERVER_MODE, Menu.NONE, "Server mode")
        popup.menu.add(Menu.NONE, MENU_NATIVE_UI, Menu.NONE, "Native UI (beta)")
        popup.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                MENU_SYNC_FOLDER -> {
                    try {
                        startActivityForResult(
                            Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
                                addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                            },
                            TREE_REQUEST
                        )
                    } catch (e: Exception) {
                        Log.e(TAG, "sync folder picker: $e")
                        toast("No folder picker available.")
                    }
                    true
                }
                MENU_EXPORT_SYNC -> {
                    exportToSyncFolder()
                    true
                }
                MENU_SERVER_MODE -> {
                    showModeDialog(firstRun = false)
                    true
                }
                MENU_NATIVE_UI -> {
                    val base = activeBase
                    if (base.isEmpty()) {
                        toast("Server not ready yet.")
                    } else {
                        startActivity(
                            Intent(this, UiActivity::class.java).apply {
                                putExtra(UiActivity.EXTRA_BASE, base)
                                putExtra(UiActivity.EXTRA_TOKEN, prefs().getString(Prefs.SERVER_TOKEN, ""))
                            },
                        )
                    }
                    true
                }
                else -> false
            }
        }
        popup.show()
    }

    private fun syncTree(): Uri? {
        val raw = getPreferences(MODE_PRIVATE).getString(Prefs.SYNC_TREE, null) ?: return null
        return try {
            Uri.parse(raw)
        } catch (_: Exception) {
            null
        }
    }

    private fun toast(msg: String) {
        runOnUiThread { Toast.makeText(this, msg, Toast.LENGTH_LONG).show() }
    }

    private fun exportToSyncFolder() {
        if (prefs().getString(Prefs.MODE, Prefs.STANDALONE) == Prefs.REMOTE) {
            toast("Export runs on the server; download it from settings instead.")
            return
        }
        val tree = syncTree()
        if (tree == null) {
            toast("Pick a sync folder first.")
            return
        }
        val base = activeBase
        if (base.isEmpty()) {
            toast("Server not ready yet.")
            return
        }
        Thread({
            try {
                val post = URL("$base/settings/export").openConnection() as HttpURLConnection
                post.requestMethod = "POST"
                post.connectTimeout = 5000
                post.readTimeout = 15000
                post.doOutput = true
                post.outputStream.use { it.write(ByteArray(0)) }
                if (post.responseCode != 200) {
                    toast("Export failed (server ${post.responseCode}).")
                    return@Thread
                }
                val site = File(File(filesDir, "bookmarks"), "site/index.html")
                if (!site.exists()) {
                    toast("Export produced no index.html.")
                    return@Thread
                }
                copyIntoTree(tree, site)
                toast("Export copied to sync folder.")
            } catch (e: Exception) {
                Log.e(TAG, "export to sync: $e")
                toast("Export to sync folder failed.")
            }
        }, "liber-export-sync").start()
    }

    private fun copyIntoTree(tree: Uri, file: File) {
        val resolver = contentResolver
        val treeId = DocumentsContract.getTreeDocumentId(tree)
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, treeId)
        resolver.query(
            children,
            arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID, OpenableColumns.DISPLAY_NAME),
            null, null, null
        )?.use { cursor ->
            val idCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
            val nameCol = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            while (cursor.moveToNext()) {
                if (cursor.getString(nameCol) == file.name) {
                    val docId = cursor.getString(idCol)
                    DocumentsContract.deleteDocument(resolver, DocumentsContract.buildDocumentUriUsingTree(tree, docId))
                }
            }
        }
        val doc = DocumentsContract.createDocument(resolver, children, "text/html", file.name)
            ?: throw IllegalStateException("createDocument returned null")
        resolver.openOutputStream(doc)?.use { out ->
            file.inputStream().use { it.copyTo(out) }
        } ?: throw IllegalStateException("openOutputStream returned null")
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode == TREE_REQUEST) {
            if (resultCode == RESULT_OK && data?.data != null) {
                val tree = data.data!!
                try {
                    contentResolver.takePersistableUriPermission(
                        tree,
                        Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    )
                    getPreferences(MODE_PRIVATE).edit().putString(Prefs.SYNC_TREE, tree.toString()).apply()
                    toast("Sync folder set.")
                } catch (e: Exception) {
                    Log.e(TAG, "persist tree permission: $e")
                    toast("Could not keep access to that folder.")
                }
            } else {
                toast("Sync folder unchanged.")
            }
            return
        }
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
        private const val TREE_REQUEST = 1002
        private const val MENU_SYNC_FOLDER = 2001
        private const val MENU_EXPORT_SYNC = 2002
        private const val MENU_SERVER_MODE = 2003
        private const val MENU_NATIVE_UI = 2004
        private const val MAX_LOAD_ATTEMPTS = 8
        private const val LOAD_RETRY_DELAY_MS = 1000L
    }
}
