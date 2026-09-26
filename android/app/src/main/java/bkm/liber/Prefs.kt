package bkm.liber

internal object Prefs {
    // Stored in the MainActivity activity-local preferences file ("MainActivity");
    // UiActivity reaches the same file via getSharedPreferences("MainActivity", ...).
    const val SYNC_TREE = "sync_tree"
    const val MODE = "mode"
    const val SERVER_URL = "server_url"
    const val SERVER_TOKEN = "server_token"
    const val STANDALONE = "standalone"
    const val REMOTE = "remote"
    const val UI_MODE = "ui_mode"
    const val NATIVE = "native"
    const val WEBVIEW = "webview"
}
