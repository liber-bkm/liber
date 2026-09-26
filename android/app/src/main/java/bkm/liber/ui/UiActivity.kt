package bkm.liber.ui

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.List
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import bkm.liber.api.ApiAttachment
import bkm.liber.api.ApiBookmark
import bkm.liber.api.ApiCheckRow
import bkm.liber.api.ApiFolder
import bkm.liber.api.ApiHistoryRow
import bkm.liber.api.ApiProfile
import bkm.liber.api.ApiSettings
import bkm.liber.api.ApiRule
import bkm.liber.api.ApiSuggestion
import bkm.liber.api.ApiTag
import bkm.liber.api.LiberApi
import kotlin.concurrent.thread

class UiActivity : ComponentActivity() {

    companion object {
        const val EXTRA_BASE = "base"
        const val EXTRA_TOKEN = "token"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val base = intent.getStringExtra(EXTRA_BASE).orEmpty()
        val token = intent.getStringExtra(EXTRA_TOKEN).orEmpty()
        if (base.isEmpty()) {
            Toast.makeText(this, "No server connected.", Toast.LENGTH_LONG).show()
            finish()
            return
        }
        setContent {
            LiberTheme {
                LiberNav(api = LiberApi(base, token))
            }
        }
    }
}

private sealed interface Screen {
    data object List : Screen
    data class Detail(val id: Int) : Screen
    data object Add : Screen
    data class Edit(val id: Int) : Screen
    data object Tags : Screen
    data object Folders : Screen
    data object Rules : Screen
    data object Check : Screen
    data object Settings : Screen
    data class Saved(val id: Int, val kind: String, val label: String) : Screen
    data object Profiles : Screen
    data object History : Screen
    data object Library : Screen
}

@Composable
fun LiberNav(api: LiberApi) {
    var stack by remember { mutableStateOf(listOf<Screen>(Screen.List)) }
    val push = { s: Screen -> stack = stack + s }
    val pop = { if (stack.size > 1) stack = stack.dropLast(1) }
    val popToList = { stack = listOf<Screen>(Screen.List) }
    BackHandler(enabled = stack.size > 1) { pop() }
    when (val top = stack.last()) {
        is Screen.List -> SearchScreen(
            api = api,
            onOpenDetail = { push(Screen.Detail(it)) },
            onOpenAdd = { push(Screen.Add) },
            onOpenTags = { push(Screen.Tags) },
            onOpenCheck = { push(Screen.Check) },
            onOpenSettings = { push(Screen.Settings) },
            onOpenHistory = { push(Screen.History) },
        )
        is Screen.Detail -> DetailScreen(
            api = api,
            id = top.id,
            onBack = { pop() },
            onOpenEdit = { push(Screen.Edit(top.id)) },
            onDeleted = { pop() },
            onOpenSaved = { kind, label -> push(Screen.Saved(top.id, kind, label)) },
        )
        is Screen.Add -> AddScreen(
            api = api,
            onBack = { pop() },
            onAdded = { pop() },
        )
        is Screen.Edit -> EditScreen(
            api = api,
            id = top.id,
            onBack = { pop() },
            onSaved = { pop() },
        )
        is Screen.Tags -> TagsScreen(
            api = api,
            onBack = { pop() },
            onOpenFolders = { push(Screen.Folders) },
            onOpenRules = { push(Screen.Rules) },
        )
        is Screen.Folders -> FoldersScreen(
            api = api,
            onBack = { pop() },
        )
        is Screen.Rules -> RulesScreen(
            api = api,
            onBack = { pop() },
        )
        is Screen.Check -> CheckScreen(
            api = api,
            onBack = { pop() },
        )
        is Screen.Settings -> SettingsScreen(
            api = api,
            onBack = { pop() },
            onOpenProfiles = { push(Screen.Profiles) },
            onOpenLibrary = { push(Screen.Library) },
        )
        is Screen.Profiles -> ProfilesScreen(
            api = api,
            onBack = { pop() },
            onSwitched = { popToList() },
        )
        is Screen.History -> HistoryScreen(
            api = api,
            onBack = { pop() },
            onOpenDetail = { push(Screen.Detail(it)) },
        )
        is Screen.Library -> LibraryScreen(
            api = api,
            onBack = { pop() },
        )
        is Screen.Saved -> SavedScreen(
            api = api,
            id = top.id,
            kind = top.kind,
            label = top.label,
            onBack = { pop() },
        )
    }
}

private fun <T> runApi(main: Handler, call: () -> T, done: (Result<T>) -> Unit) {
    thread {
        val result = try {
            Result.success(call())
        } catch (e: Exception) {
            Result.failure<T>(e)
        }
        main.post { done(result) }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LiberTopBar(
    title: String,
    onBack: (() -> Unit)?,
    actions: @Composable RowScope.() -> Unit = {},
) {
    TopAppBar(
        title = { Text(title) },
        navigationIcon = {
            onBack?.let {
                IconButton(onClick = it) {
                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                }
            }
        },
        actions = actions,
    )
}

@Composable
fun CountChip(text: String) {
    AssistChip(onClick = {}, label = { Text(text) })
}

@Composable
fun ErrorBlock(error: String, onRetry: (() -> Unit)? = null) {
    Text(
        text = error,
        color = MaterialTheme.colorScheme.error,
        modifier = Modifier.padding(top = 16.dp),
    )
    onRetry?.let {
        Button(onClick = it, modifier = Modifier.padding(top = 8.dp)) {
            Text("Retry")
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SearchScreen(
    api: LiberApi,
    onOpenDetail: (Int) -> Unit,
    onOpenAdd: () -> Unit,
    onOpenTags: () -> Unit,
    onOpenCheck: () -> Unit,
    onOpenSettings: () -> Unit,
    onOpenHistory: () -> Unit,
) {
    var query by remember { mutableStateOf("") }
    var loading by remember { mutableStateOf(false) }
    var loaded by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var total by remember { mutableStateOf(0) }
    var results by remember { mutableStateOf(listOf<ApiBookmark>()) }
    var scopes by remember { mutableStateOf(setOf("n", "u", "t", "d", "f")) }
    var deep by remember { mutableStateOf(false) }
    var sort by remember { mutableStateOf("") }
    var sortOpen by remember { mutableStateOf(false) }
    var selecting by remember { mutableStateOf(false) }
    var selected by remember { mutableStateOf(setOf<Int>()) }
    var bulkDialog by remember { mutableStateOf<String?>(null) }
    var bulkTagsText by remember { mutableStateOf("") }
    var bulkFolderText by remember { mutableStateOf("") }
    var bulkCount by remember { mutableStateOf(0) }
    var bulkBusy by remember { mutableStateOf(false) }
    val main = Handler(Looper.getMainLooper())
    val sortOptions = listOf(
        "Relevance" to "",
        "Newest" to "newest",
        "Oldest" to "oldest",
        "Visited" to "visited",
        "Title" to "title",
    )
    val scopeLabels = listOf(
        "n" to "Title",
        "u" to "URL",
        "t" to "Tags",
        "d" to "Notes",
        "f" to "Folder",
    )

    fun runSearch() {
        val q = query
        val scope = scopeLabels.map { it.first }.filter { scopes.contains(it) }.joinToString("")
        val isDeep = deep
        val sortMode = sort
        loading = true
        error = null
        runApi(main, { api.list(q, scope = scope, deep = isDeep, sort = sortMode, page = 1) }) { res ->
            loading = false
            loaded = true
            res.onSuccess {
                total = it.total
                results = it.bookmarks
            }.onFailure {
                error = it.message ?: "request failed"
            }
        }
    }

    androidx.compose.runtime.LaunchedEffect(Unit) { runSearch() }

    fun toggleScope(letter: String) {
        scopes = if (scopes.contains(letter)) {
            (scopes - letter).ifEmpty { setOf(letter) }
        } else {
            scopes + letter
        }
    }

    fun toggleSelect(id: Int) {
        selected = if (selected.contains(id)) selected - id else selected + id
    }

    fun exitSelection() {
        selecting = false
        selected = emptySet()
        bulkDialog = null
    }

    fun afterBulk() {
        exitSelection()
        runSearch()
    }

    fun submitBulkDelete(confirmed: Boolean) {
        val ids = selected
        bulkBusy = true
        error = null
        runApi(main, { api.bulkDelete(ids, confirmed) }) { res ->
            bulkBusy = false
            res.onSuccess {
                bulkDialog = null
                afterBulk()
            }.onFailure { e ->
                val needed = e as? LiberApi.ConfirmRequired
                if (needed != null && !confirmed) {
                    bulkCount = needed.count
                    bulkDialog = "delete"
                } else {
                    bulkDialog = null
                    error = e.message ?: "delete failed"
                }
            }
        }
    }

    fun submitBulkTags() {
        val ids = selected
        val tagList = bulkTagsText.split(",").map { it.trim() }.filter { it.isNotEmpty() }
        bulkBusy = true
        error = null
        runApi(main, { api.bulkTags(ids, tagList) }) { res ->
            bulkBusy = false
            res.onSuccess {
                bulkDialog = null
                afterBulk()
            }.onFailure {
                error = it.message ?: "update failed"
            }
        }
    }

    fun submitBulkFolder() {
        val ids = selected
        val f = bulkFolderText
        bulkBusy = true
        error = null
        runApi(main, { api.bulkFolder(ids, f) }) { res ->
            bulkBusy = false
            res.onSuccess {
                bulkDialog = null
                afterBulk()
            }.onFailure {
                error = it.message ?: "move failed"
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        if (selecting) {
            LiberTopBar(
                title = "${selected.size} selected",
                onBack = { exitSelection() },
            )
        } else {
            LiberTopBar(title = "liber", onBack = null) {
                IconButton(onClick = onOpenTags) {
                    Icon(Icons.Filled.List, contentDescription = "Tags")
                }
                IconButton(onClick = onOpenCheck) {
                    Icon(Icons.Filled.Check, contentDescription = "Check")
                }
                IconButton(onClick = onOpenSettings) {
                    Icon(Icons.Filled.Settings, contentDescription = "Settings")
                }
            }
        }
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Search...") },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = { runSearch() }) {
                Icon(Icons.Filled.Search, contentDescription = "Search")
            }
        }
        LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(scopeLabels.size) { i ->
                val (letter, label) = scopeLabels[i]
                FilterChip(
                    selected = scopes.contains(letter),
                    onClick = { toggleScope(letter) },
                    label = { Text(label) },
                )
            }
            item {
                FilterChip(
                    selected = deep,
                    onClick = { deep = !deep },
                    label = { Text("Deep") },
                )
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(modifier = Modifier.weight(1f)) {
                OutlinedButton(onClick = { sortOpen = !sortOpen }) {
                    Text("Sort: ${sortOptions.first { it.second == sort }.first}")
                }
                DropdownMenu(
                    expanded = sortOpen,
                    onDismissRequest = { sortOpen = false },
                ) {
                    sortOptions.forEach { (label, value) ->
                        DropdownMenuItem(
                            text = { Text(label) },
                            onClick = {
                                sort = value
                                sortOpen = false
                                runSearch()
                            },
                        )
                    }
                }
            }
            FilledTonalButton(onClick = onOpenAdd) {
                Icon(Icons.Filled.Add, contentDescription = null)
                Text("Add", modifier = Modifier.padding(start = 4.dp))
            }
            OutlinedButton(onClick = onOpenHistory) {
                Text("History")
            }
        }
        when {
            loading && !loaded -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && !loaded -> ErrorBlock(error = error ?: "")
            else -> {
                if (loading) {
                    CircularProgressIndicator(modifier = Modifier.padding(top = 8.dp))
                }
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Text(
                    text = "$total bookmark(s)",
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                )
                if (results.isEmpty() && !loading) {
                    Text(if (loaded) "No bookmarks match." else "Loading...")
                }
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(results, key = { it.id }) { b ->
                        ElevatedCard(
                            modifier = Modifier.fillMaxWidth().combinedClickable(
                                onClick = {
                                    if (selecting) toggleSelect(b.id) else onOpenDetail(b.id)
                                },
                                onLongClick = {
                                    if (!selecting) {
                                        selecting = true
                                        toggleSelect(b.id)
                                    }
                                },
                            ),
                        ) {
                            Row(modifier = Modifier.padding(12.dp)) {
                                if (selecting) {
                                    Checkbox(
                                        checked = selected.contains(b.id),
                                        onCheckedChange = { toggleSelect(b.id) },
                                    )
                                }
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(b.title, style = MaterialTheme.typography.titleMedium)
                                    Text(b.url, style = MaterialTheme.typography.bodySmall)
                                    if (b.folder.isNotEmpty() || b.tags.isNotEmpty()) {
                                        Text(
                                            (listOf(b.folder) + b.tags).filter { it.isNotEmpty() }
                                                .joinToString(" · "),
                                            style = MaterialTheme.typography.labelSmall,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
                if (selecting && selected.isNotEmpty()) {
                    ElevatedCard(modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.padding(8.dp),
                        ) {
                            TextButton(
                                onClick = { submitBulkDelete(confirmed = false) },
                                enabled = !bulkBusy,
                            ) {
                                Text("Delete")
                            }
                            TextButton(
                                onClick = {
                                    bulkTagsText = ""
                                    bulkDialog = "tags"
                                },
                                enabled = !bulkBusy,
                            ) {
                                Text("Tags")
                            }
                            TextButton(
                                onClick = {
                                    bulkFolderText = ""
                                    bulkDialog = "folder"
                                },
                                enabled = !bulkBusy,
                            ) {
                                Text("Move")
                            }
                        }
                    }
                }
            }
        }
        }
    }

    if (bulkDialog == "delete") {
        AlertDialog(
            onDismissRequest = { bulkDialog = null },
            title = { Text("Delete bookmarks?") },
            text = { Text("Delete $bulkCount bookmark(s)? This cannot be undone.") },
            confirmButton = {
                TextButton(
                    onClick = { submitBulkDelete(confirmed = true) },
                    enabled = !bulkBusy,
                ) {
                    Text(if (bulkBusy) "Deleting..." else "Delete")
                }
            },
            dismissButton = {
                TextButton(onClick = { bulkDialog = null }) {
                    Text("Cancel")
                }
            },
        )
    }
    if (bulkDialog == "tags") {
        AlertDialog(
            onDismissRequest = { bulkDialog = null },
            title = { Text("Set tags") },
            text = {
                Column {
                    Text("Replaces tags on ${selected.size} bookmark(s).")
                    OutlinedTextField(
                        value = bulkTagsText,
                        onValueChange = { bulkTagsText = it },
                        label = { Text("Tags, comma separated") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = { submitBulkTags() }, enabled = !bulkBusy) {
                    Text(if (bulkBusy) "Saving..." else "Save")
                }
            },
            dismissButton = {
                TextButton(onClick = { bulkDialog = null }) {
                    Text("Cancel")
                }
            },
        )
    }
    if (bulkDialog == "folder") {
        AlertDialog(
            onDismissRequest = { bulkDialog = null },
            title = { Text("Move to folder") },
            text = {
                Column {
                    Text("Moves ${selected.size} bookmark(s). Empty means root.")
                    OutlinedTextField(
                        value = bulkFolderText,
                        onValueChange = { bulkFolderText = it },
                        label = { Text("Folder") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = { submitBulkFolder() }, enabled = !bulkBusy) {
                    Text(if (bulkBusy) "Saving..." else "Move")
                }
            },
            dismissButton = {
                TextButton(onClick = { bulkDialog = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun DetailScreen(
    api: LiberApi,
    id: Int,
    onBack: () -> Unit,
    onOpenEdit: () -> Unit,
    onDeleted: () -> Unit,
    onOpenSaved: (kind: String, label: String) -> Unit,
) {
    val context = LocalContext.current
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var bookmark by remember { mutableStateOf<ApiBookmark?>(null) }
    var showDelete by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    var opening by remember { mutableStateOf<String?>(null) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        error = null
        runApi(main, { api.get(id) }) { res ->
            loading = false
            res.onSuccess { bookmark = it }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(id) { load() }

    fun submitDelete() {
        deleting = true
        error = null
        runApi(main, { api.delete(id, confirmed = true) }) { res ->
            deleting = false
            res.onSuccess { onDeleted() }.onFailure {
                showDelete = false
                error = it.message ?: "delete failed"
            }
        }
    }

    fun openAttachment(name: String, n: Int) {
        opening = name
        error = null
        runApi(main, { api.downloadAttachment(id, n) }) { res ->
            opening = null
            res.onSuccess { bytes ->
                try {
                    val dir = java.io.File(context.cacheDir, "liber-attachments").apply { mkdirs() }
                    val safe = name.replace(Regex("[^A-Za-z0-9._-]"), "_").takeLast(64)
                    val file = java.io.File(dir, "${id}-${n}-$safe")
                    file.writeBytes(bytes)
                    val uri = androidx.core.content.FileProvider.getUriForFile(
                        context, "bkm.liber.fileprovider", file,
                    )
                    val type = context.contentResolver.getType(uri)
                        ?: android.webkit.MimeTypeMap.getSingleton()
                            .getMimeTypeFromExtension(file.extension.lowercase())
                        ?: "*/*"
                    val view = Intent(Intent.ACTION_VIEW).apply {
                        setDataAndType(uri, type)
                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    context.startActivity(Intent.createChooser(view, name))
                } catch (e: Exception) {
                    error = e.message ?: "open failed"
                }
            }.onFailure {
                error = it.message ?: "download failed"
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Detail", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null -> ErrorBlock(error = error ?: "", onRetry = { load() })
            else -> bookmark?.let { b ->
                ElevatedCard(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(b.title, style = MaterialTheme.typography.headlineSmall)
                        Text(b.url, style = MaterialTheme.typography.bodyMedium)
                        if (b.description.isNotEmpty()) {
                            Text(b.description, modifier = Modifier.padding(top = 8.dp))
                        }
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.padding(top = 8.dp),
                        ) {
                            if (b.folder.isNotEmpty()) {
                                CountChip("Folder: ${b.folder}")
                            }
                            if (b.tags.isNotEmpty()) {
                                CountChip("Tags: ${b.tags.joinToString(", ")}")
                            }
                        }
                        Text(
                            "md: ${if (b.hasMarkdown) "yes" else "no"} · archive: ${if (b.hasArchive) "yes" else "no"}" +
                                " · opened ${b.openCount}x",
                            style = MaterialTheme.typography.labelSmall,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilledTonalButton(
                        onClick = {
                            runApi(main, { api.open(id) }) { res ->
                                res.onSuccess { url ->
                                    context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                                }.onFailure {
                                    error = it.message ?: "open failed"
                                }
                            }
                        },
                        modifier = Modifier.padding(top = 8.dp),
                    ) {
                        Icon(Icons.Filled.PlayArrow, contentDescription = null)
                        Text("Open", modifier = Modifier.padding(start = 4.dp))
                    }
                    OutlinedButton(
                        onClick = onOpenEdit,
                        modifier = Modifier.padding(top = 8.dp),
                    ) {
                        Icon(Icons.Filled.Edit, contentDescription = null)
                        Text("Edit", modifier = Modifier.padding(start = 4.dp))
                    }
                    OutlinedButton(
                        onClick = { showDelete = true },
                        enabled = !deleting,
                        modifier = Modifier.padding(top = 8.dp),
                    ) {
                        Icon(Icons.Filled.Delete, contentDescription = null)
                        Text("Delete", modifier = Modifier.padding(start = 4.dp))
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(
                        onClick = { onOpenSaved("card", "Card") },
                        modifier = Modifier.padding(top = 8.dp),
                    ) {
                        Text("Card")
                    }
                    if (b.hasArchive) {
                        OutlinedButton(
                            onClick = { onOpenSaved("archive", "Archive") },
                            modifier = Modifier.padding(top = 8.dp),
                        ) {
                            Text("Archive")
                        }
                    }
                    if (b.hasMarkdown) {
                        OutlinedButton(
                            onClick = { onOpenSaved("markdown", "Notes") },
                            modifier = Modifier.padding(top = 8.dp),
                        ) {
                            Text("Notes")
                        }
                    }
                }
                if (b.attachments.isNotEmpty()) {
                    Text(
                        "Attachments",
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(top = 16.dp),
                    )
                    b.attachments.forEachIndexed { i, at ->
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            TextButton(
                                onClick = { openAttachment(at.name, i + 1) },
                                enabled = opening == null,
                            ) {
                                Text(
                                    if (opening == at.name) "Opening..." else at.name,
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                            }
                        }
                    }
                }
            }
        }
        }
    }

    if (showDelete && bookmark != null) {
        AlertDialog(
            onDismissRequest = { showDelete = false },
            title = { Text("Delete bookmark?") },
            text = { Text("Delete \"${bookmark?.title}\"? This cannot be undone.") },
            confirmButton = {
                TextButton(onClick = { submitDelete() }, enabled = !deleting) {
                    Text(if (deleting) "Deleting..." else "Delete")
                }
            },
            dismissButton = {
                TextButton(onClick = { showDelete = false }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun AddScreen(api: LiberApi, onBack: () -> Unit, onAdded: () -> Unit) {
    val context = LocalContext.current
    var url by remember { mutableStateOf("") }
    var title by remember { mutableStateOf("") }
    var wantMarkdown by remember { mutableStateOf(false) }
    var picked by remember { mutableStateOf(listOf<android.net.Uri>()) }
    var createdId by remember { mutableStateOf<Int?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var saving by remember { mutableStateOf(false) }
    var pendingDup by remember { mutableStateOf<ApiBookmark?>(null) }
    var pendingTitle by remember { mutableStateOf("") }
    val main = Handler(Looper.getMainLooper())

    val picker = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.OpenMultipleDocuments(),
    ) { uris ->
        picked = picked + uris
    }

    fun fileName(uri: android.net.Uri): String {
        context.contentResolver.query(uri, null, null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val idx = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                if (idx >= 0) return c.getString(idx)
            }
        }
        return uri.lastPathSegment?.substringAfterLast("/") ?: "attachment"
    }

    fun submit(confirmed: Boolean) {
        saving = true
        error = null
        val u = url
        val t = title
        val md = wantMarkdown
        val files = picked.toList()
        runApi(
            main,
            {
                val created = api.add(u, t, confirmed, md)
                createdId = created.id
                for (uri in files) {
                    val bytes = context.contentResolver.openInputStream(uri)?.readBytes()
                        ?: throw java.io.IOException("cannot read file")
                    api.uploadAttachment(created.id, fileName(uri), bytes)
                }
                created
            },
        ) { res ->
            saving = false
            res.onSuccess { onAdded() }.onFailure { e ->
                val dup = (e as? LiberApi.Duplicate)?.existing
                if (dup != null && !confirmed && createdId == null) {
                    pendingDup = dup
                    pendingTitle = t
                } else {
                    error = e.message ?: "add failed"
                }
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Add bookmark", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        OutlinedTextField(
            value = url,
            onValueChange = { url = it },
            placeholder = { Text("https://example.com") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        OutlinedTextField(
            value = title,
            onValueChange = { title = it },
            placeholder = { Text("Title (optional)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Checkbox(
                checked = wantMarkdown,
                onCheckedChange = { wantMarkdown = it },
            )
            Text(
                "Markdown notes",
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(top = 12.dp),
            )
        }
        if (picked.isNotEmpty()) {
            Text(
                picked.map { fileName(it) }.joinToString(", "),
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = { picker.launch(arrayOf("*/*")) },
                enabled = !saving,
            ) {
                Text("Attach files")
            }
            if (picked.isNotEmpty()) {
                TextButton(onClick = { picked = emptyList() }, enabled = !saving) {
                    Text("Clear")
                }
            }
        }
        if (error != null) {
            Text(
                text = error ?: "",
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
        val doneId = createdId
        Button(
            onClick = { if (doneId != null) onAdded() else submit(confirmed = false) },
            enabled = !saving && (doneId != null || url.isNotBlank()),
            modifier = Modifier.padding(top = 8.dp),
        ) {
            Text(if (saving) "Saving..." else if (doneId != null) "Done" else "Add")
        }
        }
    }

    pendingDup?.let { dup ->
        AlertDialog(
            onDismissRequest = { pendingDup = null },
            title = { Text("Possible duplicate") },
            text = { Text("Already bookmarked as \"${dup.title}\" (${dup.url}). Add anyway?") },
            confirmButton = {
                TextButton(onClick = { pendingDup = null; submit(confirmed = true) }) {
                    Text("Add anyway")
                }
            },
            dismissButton = {
                TextButton(onClick = { pendingDup = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun EditScreen(api: LiberApi, id: Int, onBack: () -> Unit, onSaved: () -> Unit) {
    val context = LocalContext.current
    var loading by remember { mutableStateOf(true) }
    var loadError by remember { mutableStateOf<String?>(null) }
    var title by remember { mutableStateOf("") }
    var url by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var tags by remember { mutableStateOf("") }
    var folder by remember { mutableStateOf("") }
    var wantMarkdown by remember { mutableStateOf(false) }
    var notes by remember { mutableStateOf("") }
    var attachments by remember { mutableStateOf(listOf<ApiAttachment>()) }
    var error by remember { mutableStateOf<String?>(null) }
    var saving by remember { mutableStateOf(false) }
    val main = Handler(Looper.getMainLooper())

    val picker = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.OpenMultipleDocuments(),
    ) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        saving = true
        error = null
        runApi(
            main,
            {
                for (uri in uris) {
                    val bytes = context.contentResolver.openInputStream(uri)?.readBytes()
                        ?: throw java.io.IOException("cannot read file")
                    var name = uri.lastPathSegment?.substringAfterLast("/") ?: "attachment"
                    context.contentResolver.query(uri, null, null, null, null)?.use { c ->
                        if (c.moveToFirst()) {
                            val idx = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                            if (idx >= 0) name = c.getString(idx)
                        }
                    }
                    api.uploadAttachment(id, name, bytes)
                }
                api.get(id)
            },
        ) { res ->
            saving = false
            res.onSuccess { b -> attachments = b.attachments }
                .onFailure { error = it.message ?: "upload failed" }
        }
    }

    fun load() {
        loading = true
        loadError = null
        runApi(
            main,
            {
                val b = api.get(id)
                val raw = if (b.hasMarkdown) {
                    try {
                        api.rawMarkdown(id)
                    } catch (e: Exception) {
                        ""
                    }
                } else {
                    ""
                }
                b to raw
            },
        ) { res ->
            loading = false
            res.onSuccess { (b, raw) ->
                title = b.title
                url = b.url
                description = b.description
                tags = b.tags.joinToString(", ")
                folder = b.folder
                wantMarkdown = b.hasMarkdown
                notes = raw
                attachments = b.attachments
            }.onFailure {
                loadError = it.message ?: "request failed"
            }
        }
    }

    androidx.compose.runtime.LaunchedEffect(id) { load() }

    fun submit() {
        saving = true
        error = null
        val t = title
        val u = url
        val d = description
        val f = folder
        val tagList = tags.split(",").map { it.trim() }.filter { it.isNotEmpty() }
        val text = if (wantMarkdown) notes else null
        runApi(main, { api.update(id, t, u, d, tagList, f, text) }) { res ->
            saving = false
            res.onSuccess { onSaved() }.onFailure {
                error = it.message ?: "save failed"
            }
        }
    }

    fun removeAttachment(name: String, n: Int) {
        saving = true
        error = null
        runApi(
            main,
            {
                api.deleteAttachment(id, n)
                api.get(id)
            },
        ) { res ->
            saving = false
            res.onSuccess { b -> attachments = b.attachments }
                .onFailure { error = it.message ?: "remove failed" }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Edit bookmark", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            loadError != null -> ErrorBlock(error = loadError ?: "", onRetry = { load() })
            else -> {
                OutlinedTextField(
                    value = title,
                    onValueChange = { title = it },
                    placeholder = { Text("Title") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    placeholder = { Text("https://example.com") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                OutlinedTextField(
                    value = description,
                    onValueChange = { description = it },
                    placeholder = { Text("Description (optional)") },
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                OutlinedTextField(
                    value = tags,
                    onValueChange = { tags = it },
                    placeholder = { Text("Tags, comma separated (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                OutlinedTextField(
                    value = folder,
                    onValueChange = { folder = it },
                    placeholder = { Text("Folder (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Checkbox(
                        checked = wantMarkdown,
                        onCheckedChange = { wantMarkdown = it },
                    )
                    Text(
                        "Markdown notes",
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(top = 12.dp),
                    )
                }
                if (wantMarkdown) {
                    OutlinedTextField(
                        value = notes,
                        onValueChange = { notes = it },
                        placeholder = { Text("Notes (markdown)") },
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                        minLines = 6,
                    )
                }
                Text(
                    "Attachments",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                if (attachments.isEmpty()) {
                    Text("None.", style = MaterialTheme.typography.bodySmall)
                }
                attachments.forEachIndexed { i, at ->
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            at.name,
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.weight(1f).padding(top = 12.dp),
                        )
                        IconButton(
                            onClick = { removeAttachment(at.name, i + 1) },
                            enabled = !saving,
                        ) {
                            Icon(Icons.Filled.Delete, contentDescription = "Remove")
                        }
                    }
                }
                OutlinedButton(
                    onClick = { picker.launch(arrayOf("*/*")) },
                    enabled = !saving,
                    modifier = Modifier.padding(top = 4.dp),
                ) {
                    Text("Attach files")
                }
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Button(
                    onClick = { submit() },
                    enabled = !saving && title.isNotBlank() && url.isNotBlank(),
                    modifier = Modifier.padding(top = 8.dp),
                ) {
                    Text(if (saving) "Saving..." else "Save")
                }
            }
        }
        }
    }
}

@Composable
fun TagsScreen(api: LiberApi, onBack: () -> Unit, onOpenFolders: () -> Unit, onOpenRules: () -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var tags by remember { mutableStateOf(listOf<ApiTag>()) }
    var mutating by remember { mutableStateOf(false) }
    var renameTarget by remember { mutableStateOf<ApiTag?>(null) }
    var renameValue by remember { mutableStateOf("") }
    var deleteTarget by remember { mutableStateOf<ApiTag?>(null) }
    var deleteCount by remember { mutableStateOf(0) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        error = null
        runApi(main, { api.tags() }) { res ->
            loading = false
            res.onSuccess { tags = it }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(Unit) { load() }

    fun submitRename() {
        val target = renameTarget ?: return
        val new = renameValue
        mutating = true
        error = null
        runApi(main, { api.renameTag(target.name, new) }) { res ->
            mutating = false
            res.onSuccess {
                renameTarget = null
                load()
            }.onFailure {
                error = it.message ?: "rename failed"
            }
        }
    }

    fun submitDelete(confirmed: Boolean) {
        val target = deleteTarget ?: return
        mutating = true
        error = null
        runApi(main, { api.deleteTag(target.name, confirmed) }) { res ->
            mutating = false
            res.onSuccess {
                deleteTarget = null
                load()
            }.onFailure { e ->
                val needed = e as? LiberApi.ConfirmRequired
                if (needed != null && !confirmed) {
                    deleteCount = needed.count
                } else {
                    error = e.message ?: "delete failed"
                }
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Tags", onBack = onBack) {
            IconButton(onClick = onOpenFolders) {
                Icon(Icons.Filled.Home, contentDescription = "Folders")
            }
            IconButton(onClick = onOpenRules) {
                Icon(Icons.Filled.Refresh, contentDescription = "Rules")
            }
        }
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && tags.isEmpty() -> ErrorBlock(error = error ?: "", onRetry = { load() })
            else -> {
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Text(
                    text = "${tags.size} tag(s)",
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                )
                if (tags.isEmpty()) {
                    Text("No tags yet.")
                }
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(tags, key = { it.name }) { t ->
                        ElevatedCard(modifier = Modifier.fillMaxWidth()) {
                            Row(
                                modifier = Modifier.fillMaxWidth().padding(12.dp),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(t.name, style = MaterialTheme.typography.titleMedium)
                                    CountChip("${t.count} bookmark(s)")
                                }
                                IconButton(
                                    onClick = {
                                        renameTarget = t
                                        renameValue = t.name
                                    },
                                    enabled = !mutating,
                                ) {
                                    Icon(Icons.Filled.Edit, contentDescription = "Rename")
                                }
                                IconButton(
                                    onClick = {
                                        deleteTarget = t
                                        deleteCount = t.count
                                        submitDelete(confirmed = false)
                                    },
                                    enabled = !mutating,
                                ) {
                                    Icon(Icons.Filled.Delete, contentDescription = "Delete")
                                }
                            }
                        }
                    }
                }
            }
        }
        }
    }

    renameTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { renameTarget = null },
            title = { Text("Rename tag") },
            text = {
                OutlinedTextField(
                    value = renameValue,
                    onValueChange = { renameValue = it },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            confirmButton = {
                TextButton(
                    onClick = { submitRename() },
                    enabled = !mutating && renameValue.isNotBlank() &&
                        !renameValue.equals(target.name, ignoreCase = true),
                ) {
                    Text(if (mutating) "Saving..." else "Rename")
                }
            },
            dismissButton = {
                TextButton(onClick = { renameTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }

    deleteTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Delete tag?") },
            text = { Text("Remove \"${target.name}\" from $deleteCount bookmark(s)?") },
            confirmButton = {
                TextButton(onClick = { submitDelete(confirmed = true) }, enabled = !mutating) {
                    Text(if (mutating) "Deleting..." else "Delete")
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun FoldersScreen(api: LiberApi, onBack: () -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var folders by remember { mutableStateOf(listOf<ApiFolder>()) }
    var mutating by remember { mutableStateOf(false) }
    var renameTarget by remember { mutableStateOf<ApiFolder?>(null) }
    var renameValue by remember { mutableStateOf("") }
    var deleteTarget by remember { mutableStateOf<ApiFolder?>(null) }
    var deleteCount by remember { mutableStateOf(0) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        error = null
        runApi(main, { api.folders() }) { res ->
            loading = false
            res.onSuccess { folders = it }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(Unit) { load() }

    fun submitRename() {
        val target = renameTarget ?: return
        val new = renameValue
        mutating = true
        error = null
        runApi(main, { api.renameFolder(target.name, new) }) { res ->
            mutating = false
            res.onSuccess {
                renameTarget = null
                load()
            }.onFailure {
                error = it.message ?: "rename failed"
            }
        }
    }

    fun submitDelete(confirmed: Boolean) {
        val target = deleteTarget ?: return
        mutating = true
        error = null
        runApi(main, { api.deleteFolder(target.name, confirmed) }) { res ->
            mutating = false
            res.onSuccess {
                deleteTarget = null
                load()
            }.onFailure { e ->
                val needed = e as? LiberApi.ConfirmRequired
                if (needed != null && !confirmed) {
                    deleteCount = needed.count
                } else {
                    error = e.message ?: "delete failed"
                }
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Folders", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && folders.isEmpty() -> ErrorBlock(error = error ?: "", onRetry = { load() })
            else -> {
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Text(
                    text = "${folders.size} folder(s)",
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                )
                if (folders.isEmpty()) {
                    Text("Everything is at the root.")
                }
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(folders, key = { it.name }) { f ->
                        ElevatedCard(modifier = Modifier.fillMaxWidth()) {
                            Row(
                                modifier = Modifier.fillMaxWidth().padding(12.dp),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        f.display.ifEmpty { "/" },
                                        style = MaterialTheme.typography.titleMedium,
                                    )
                                    CountChip("${f.count} bookmark(s)")
                                }
                                IconButton(
                                    onClick = {
                                        renameTarget = f
                                        renameValue = f.name
                                    },
                                    enabled = !mutating && f.name.isNotEmpty(),
                                ) {
                                    Icon(Icons.Filled.Edit, contentDescription = "Rename")
                                }
                                IconButton(
                                    onClick = {
                                        deleteTarget = f
                                        deleteCount = f.count
                                        submitDelete(confirmed = false)
                                    },
                                    enabled = !mutating && f.name.isNotEmpty(),
                                ) {
                                    Icon(Icons.Filled.Delete, contentDescription = "Delete")
                                }
                            }
                        }
                    }
                }
            }
        }
        }
    }

    renameTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { renameTarget = null },
            title = { Text("Rename folder") },
            text = {
                OutlinedTextField(
                    value = renameValue,
                    onValueChange = { renameValue = it },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            confirmButton = {
                TextButton(
                    onClick = { submitRename() },
                    enabled = !mutating && renameValue.isNotBlank() && renameValue != target.name,
                ) {
                    Text(if (mutating) "Saving..." else "Rename")
                }
            },
            dismissButton = {
                TextButton(onClick = { renameTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }

    deleteTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Delete folder?") },
            text = { Text("Move $deleteCount bookmark(s) from \"${target.display}\" back to the root?") },
            confirmButton = {
                TextButton(onClick = { submitDelete(confirmed = true) }, enabled = !mutating) {
                    Text(if (mutating) "Deleting..." else "Move to root")
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun RulesScreen(api: LiberApi, onBack: () -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var rules by remember { mutableStateOf(listOf<ApiRule>()) }
    var mutating by remember { mutableStateOf(false) }
    var match by remember { mutableStateOf("") }
    var folder by remember { mutableStateOf("") }
    var tags by remember { mutableStateOf("") }
    var learnMin by remember { mutableStateOf("3") }
    var suggestions by remember { mutableStateOf(listOf<ApiSuggestion>()) }
    var learnResult by remember { mutableStateOf<String?>(null) }
    var deleteTarget by remember { mutableStateOf<ApiRule?>(null) }
    var deleteCount by remember { mutableStateOf(0) }
    var editTarget by remember { mutableStateOf<ApiRule?>(null) }
    var editMatch by remember { mutableStateOf("") }
    var editFolder by remember { mutableStateOf("") }
    var editTags by remember { mutableStateOf("") }
    var editReapply by remember { mutableStateOf(false) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        error = null
        runApi(main, { api.rules() }) { res ->
            loading = false
            res.onSuccess { rules = it }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(Unit) { load() }

    fun submitAdd() {
        val m = match
        val f = folder
        val tagList = tags.split(",").map { it.trim() }.filter { it.isNotEmpty() }
        mutating = true
        error = null
        runApi(main, { api.createRule(m, f, tagList) }) { res ->
            mutating = false
            res.onSuccess {
                match = ""
                folder = ""
                tags = ""
                load()
            }.onFailure {
                error = it.message ?: "add failed"
            }
        }
    }

    fun submitEdit() {
        val target = editTarget ?: return
        val m = editMatch
        val f = editFolder
        val tagList = editTags.split(",").map { it.trim() }.filter { it.isNotEmpty() }
        val reapply = editReapply
        mutating = true
        error = null
        runApi(main, { api.editRule(target.id, m, f, tagList, reapply) }) { res ->
            mutating = false
            res.onSuccess { (_, n) ->
                editTarget = null
                learnResult = if (reapply) {
                    "Rule saved (reapplied to $n bookmark(s))."
                } else {
                    "Rule saved."
                }
                load()
            }.onFailure {
                error = it.message ?: "save failed"
            }
        }
    }

    fun submitApply(id: Int?) {
        mutating = true
        error = null
        learnResult = null
        runApi(main, { api.applyRules(id) }) { res ->
            mutating = false
            res.onSuccess { n ->
                learnResult = if (id == null) {
                    "Applied to $n bookmark(s)."
                } else {
                    "Rule applied to $n bookmark(s)."
                }
                load()
            }.onFailure {
                error = it.message ?: "apply failed"
            }
        }
    }

    fun submitSuggest() {
        val min = learnMin.toIntOrNull() ?: 0
        mutating = true
        error = null
        runApi(main, { api.suggestions(min) }) { res ->
            mutating = false
            res.onSuccess { suggestions = it }
                .onFailure { error = it.message ?: "suggest failed" }
        }
    }

    fun submitLearnAll() {
        val min = learnMin.toIntOrNull() ?: 0
        mutating = true
        error = null
        runApi(main, { api.learnAll(min) }) { res ->
            mutating = false
            res.onSuccess { n ->
                learnResult = "Created $n rule(s)."
                load()
            }.onFailure {
                error = it.message ?: "learn failed"
            }
        }
    }

    fun submitDelete(confirmed: Boolean) {
        val target = deleteTarget ?: return
        mutating = true
        error = null
        runApi(main, { api.deleteRule(target.id, confirmed) }) { res ->
            mutating = false
            res.onSuccess {
                deleteTarget = null
                load()
            }.onFailure { e ->
                val needed = e as? LiberApi.ConfirmRequired
                if (needed != null && !confirmed) {
                    deleteCount = needed.count
                } else {
                    error = e.message ?: "delete failed"
                }
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Rules", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && rules.isEmpty() -> ErrorBlock(error = error ?: "", onRetry = { load() })
            else -> {
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Text(
                    text = "${rules.size} rule(s)",
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                )
                if (rules.isEmpty()) {
                    Text("No rules yet.")
                }
                LazyColumn(
                    modifier = Modifier.weight(1f, fill = false),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(rules, key = { it.id }) { r ->
                        ElevatedCard(modifier = Modifier.fillMaxWidth()) {
                            Row(
                                modifier = Modifier.fillMaxWidth().padding(12.dp),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(r.match, style = MaterialTheme.typography.titleMedium)
                                    val detail = (listOf(r.folder) + r.tags)
                                        .filter { it.isNotEmpty() }.joinToString(" · ")
                                    if (detail.isNotEmpty()) {
                                        Text(detail, style = MaterialTheme.typography.bodySmall)
                                    }
                                    CountChip("applied to ${r.appliedCount} bookmark(s)")
                                }
                                IconButton(
                                    onClick = { submitApply(r.id) },
                                    enabled = !mutating,
                                ) {
                                    Icon(Icons.Filled.PlayArrow, contentDescription = "Apply")
                                }
                                IconButton(
                                    onClick = {
                                        editTarget = r
                                        editMatch = r.match
                                        editFolder = r.folder
                                        editTags = r.tags.joinToString(", ")
                                        editReapply = false
                                    },
                                    enabled = !mutating,
                                ) {
                                    Icon(Icons.Filled.Edit, contentDescription = "Edit")
                                }
                                IconButton(
                                    onClick = {
                                        deleteTarget = r
                                        deleteCount = r.appliedCount
                                        submitDelete(confirmed = false)
                                    },
                                    enabled = !mutating,
                                ) {
                                    Icon(Icons.Filled.Delete, contentDescription = "Delete")
                                }
                            }
                        }
                    }
                }
                Text(
                    text = "Add rule",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                OutlinedTextField(
                    value = match,
                    onValueChange = { match = it },
                    placeholder = { Text("Match, e.g. host:example.com") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                OutlinedTextField(
                    value = folder,
                    onValueChange = { folder = it },
                    placeholder = { Text("Folder (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                OutlinedTextField(
                    value = tags,
                    onValueChange = { tags = it },
                    placeholder = { Text("Tags, comma separated (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                Button(
                    onClick = { submitAdd() },
                    enabled = !mutating && match.isNotBlank() &&
                        (folder.isNotBlank() || tags.split(",").any { it.isNotBlank() }),
                    modifier = Modifier.padding(top = 4.dp),
                ) {
                    Text(if (mutating) "Saving..." else "Add")
                }
                Text(
                    text = "Learn from collection",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = learnMin,
                        onValueChange = { learnMin = it },
                        placeholder = { Text("Min") },
                        singleLine = true,
                        modifier = Modifier.weight(1f),
                    )
                    Button(onClick = { submitSuggest() }, enabled = !mutating) {
                        Text("Suggest")
                    }
                }
                if (suggestions.isNotEmpty()) {
                    Text(
                        suggestions.joinToString("\n") {
                            "${it.count}x ${it.host} -> ${it.folder}"
                        },
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = { submitLearnAll() }, enabled = !mutating) {
                            Text("Create all")
                        }
                        Button(onClick = { submitApply(null) }, enabled = !mutating) {
                            Text("Apply all")
                        }
                    }
                }
                if (learnResult != null) {
                    Text(
                        text = learnResult ?: "",
                        style = MaterialTheme.typography.labelMedium,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            }
        }
        }
    }

    deleteTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Delete rule?") },
            text = { Text("Delete \"${target.match}\" (applied to $deleteCount bookmark(s))? Classified bookmarks stay as-is.") },
            confirmButton = {
                TextButton(onClick = { submitDelete(confirmed = true) }, enabled = !mutating) {
                    Text(if (mutating) "Deleting..." else "Delete")
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }

    editTarget?.let {
        AlertDialog(
            onDismissRequest = { editTarget = null },
            title = { Text("Edit rule") },
            text = {
                Column {
                    OutlinedTextField(
                        value = editMatch,
                        onValueChange = { editMatch = it },
                        label = { Text("Match") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    OutlinedTextField(
                        value = editFolder,
                        onValueChange = { editFolder = it },
                        label = { Text("Folder") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    OutlinedTextField(
                        value = editTags,
                        onValueChange = { editTags = it },
                        label = { Text("Tags, comma separated") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                    )
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.padding(top = 8.dp),
                    ) {
                        Checkbox(
                            checked = editReapply,
                            onCheckedChange = { editReapply = it },
                        )
                        Text(
                            "Reapply to classified bookmarks",
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(top = 12.dp),
                        )
                    }
                }
            },
            confirmButton = {
                TextButton(
                    onClick = { submitEdit() },
                    enabled = !mutating && editMatch.isNotBlank(),
                ) {
                    Text(if (mutating) "Saving..." else "Save")
                }
            },
            dismissButton = {
                TextButton(onClick = { editTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun CheckScreen(api: LiberApi, onBack: () -> Unit) {
    var spec by remember { mutableStateOf("") }
    var stale by remember { mutableStateOf("") }
    var scanning by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var flash by remember { mutableStateOf<String?>(null) }
    var summary by remember { mutableStateOf<String?>(null) }
    var moved by remember { mutableStateOf(listOf<ApiCheckRow>()) }
    var dead by remember { mutableStateOf(listOf<ApiCheckRow>()) }
    var uncertain by remember { mutableStateOf(listOf<ApiCheckRow>()) }
    var mutating by remember { mutableStateOf(false) }
    var deleteTarget by remember { mutableStateOf<ApiCheckRow?>(null) }
    val main = Handler(Looper.getMainLooper())

    fun runScan() {
        val s = spec
        val st = stale
        scanning = true
        error = null
        flash = null
        summary = null
        moved = emptyList()
        dead = emptyList()
        uncertain = emptyList()
        runApi(main, { api.checkRun(s, 0, st) }) { res ->
            scanning = false
            res.onSuccess { out ->
                summary = "${out.ok} ok, ${out.moved.size} moved, " +
                    "${out.dead.size} dead, ${out.uncertain.size} uncertain " +
                    "(of ${out.checked} checked)"
                moved = out.moved
                dead = out.dead
                uncertain = out.uncertain
            }.onFailure {
                error = it.message ?: "scan failed"
            }
        }
    }

    fun dropRow(id: Int) {
        moved = moved.filterNot { it.id == id }
        dead = dead.filterNot { it.id == id }
        uncertain = uncertain.filterNot { it.id == id }
    }

    fun submitApply(row: ApiCheckRow, action: String, confirmed: Boolean) {
        mutating = true
        error = null
        runApi(main, { api.checkApply(row.id, action, row.target, confirmed) }) { res ->
            mutating = false
            res.onSuccess { msg ->
                flash = msg
                dropRow(row.id)
            }.onFailure { e ->
                if (e is LiberApi.ConfirmRequired && !confirmed) {
                    deleteTarget = row
                } else {
                    error = e.message ?: "apply failed"
                }
            }
        }
    }

    @Composable
    fun CheckRowView(
        row: ApiCheckRow,
        bucketColor: androidx.compose.ui.graphics.Color,
        actions: @Composable () -> Unit,
    ) {
        ElevatedCard(modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(12.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    CountChip(row.status)
                    Text(
                        "[${row.id}] ${row.title}",
                        style = MaterialTheme.typography.titleMedium,
                        color = bucketColor,
                    )
                }
                Text(row.url, style = MaterialTheme.typography.bodySmall)
                val extra = listOf(row.detail, row.target).filter { it.isNotEmpty() }
                    .joinToString(" -> ")
                if (extra.isNotEmpty()) {
                    Text(extra, style = MaterialTheme.typography.labelSmall)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    actions()
                }
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Check links", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        OutlinedTextField(
            value = spec,
            onValueChange = { spec = it },
            placeholder = { Text("ids like 1-100 (empty = all)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        OutlinedTextField(
            value = stale,
            onValueChange = { stale = it },
            placeholder = { Text("only stale, e.g. 720h (empty = all)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        FilledTonalButton(
            onClick = { runScan() },
            enabled = !scanning && !mutating,
            modifier = Modifier.padding(top = 8.dp),
        ) {
            Icon(Icons.Filled.PlayArrow, contentDescription = null)
            Text(
                if (scanning) "Scanning..." else "Run check",
                modifier = Modifier.padding(start = 4.dp),
            )
        }
        if (scanning) {
            Text(
                "Scanning can take a while on large collections.",
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
        if (error != null) {
            Text(
                text = error ?: "",
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
        if (flash != null) {
            Text(
                text = flash ?: "",
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
        if (summary != null) {
            Text(
                text = summary ?: "",
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
            )
            LazyColumn(
                modifier = Modifier.weight(1f, fill = false),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (moved.isNotEmpty()) {
                    item {
                        Text("Moved", style = MaterialTheme.typography.titleMedium)
                    }
                    items(moved, key = { it.id }) { row ->
                        CheckRowView(row, MaterialTheme.colorScheme.tertiary) {
                            IconButton(
                                onClick = { submitApply(row, "update", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Icon(Icons.Filled.Check, contentDescription = "Update URL")
                            }
                            IconButton(
                                onClick = { submitApply(row, "retitle", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Icon(Icons.Filled.Refresh, contentDescription = "URL + title")
                            }
                        }
                    }
                }
                if (dead.isNotEmpty()) {
                    item {
                        Text("Dead", style = MaterialTheme.typography.titleMedium)
                    }
                    items(dead, key = { it.id }) { row ->
                        CheckRowView(row, MaterialTheme.colorScheme.error) {
                            IconButton(
                                onClick = { submitApply(row, "delete", confirmed = false) },
                                enabled = !mutating,
                            ) {
                                Icon(Icons.Filled.Delete, contentDescription = "Delete")
                            }
                            IconButton(
                                onClick = { submitApply(row, "quarantine", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Icon(Icons.Filled.Warning, contentDescription = "Quarantine")
                            }
                        }
                    }
                }
                if (uncertain.isNotEmpty()) {
                    item {
                        Text("Uncertain", style = MaterialTheme.typography.titleMedium)
                    }
                    items(uncertain, key = { it.id }) { row ->
                        CheckRowView(row, MaterialTheme.colorScheme.onSurfaceVariant) {
                            IconButton(
                                onClick = { submitApply(row, "delete", confirmed = false) },
                                enabled = !mutating,
                            ) {
                                Icon(Icons.Filled.Delete, contentDescription = "Delete")
                            }
                            IconButton(
                                onClick = { submitApply(row, "quarantine", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Icon(Icons.Filled.Warning, contentDescription = "Quarantine")
                            }
                        }
                    }
                }
            }
        }
        }
    }

    deleteTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Delete bookmark?") },
            text = { Text("Delete [${target.id}] \"${target.title}\"?") },
            confirmButton = {
                TextButton(
                    onClick = {
                        deleteTarget = null
                        submitApply(target, "delete", confirmed = true)
                    },
                    enabled = !mutating,
                ) {
                    Text(if (mutating) "Deleting..." else "Delete")
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun SettingsScreen(api: LiberApi, onBack: () -> Unit, onOpenProfiles: () -> Unit, onOpenLibrary: () -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var settings by remember { mutableStateOf<ApiSettings?>(null) }
    var saving by remember { mutableStateOf(false) }
    val main = Handler(Looper.getMainLooper())
    val backends = listOf("auto", "single-file", "monolith", "native")

    fun load() {
        loading = true
        error = null
        runApi(main, { api.settings() }) { res ->
            loading = false
            res.onSuccess { settings = it }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(Unit) { load() }

    fun selectBackend(backend: String) {
        saving = true
        error = null
        runApi(main, { api.setBackend(backend) }) { res ->
            saving = false
            res.onSuccess { load() }
                .onFailure { error = it.message ?: "save failed" }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Settings", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && settings == null -> ErrorBlock(error = error ?: "", onRetry = { load() })
            else -> settings?.let { s ->
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                ElevatedCard(modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text("Collection", style = MaterialTheme.typography.titleMedium)
                        Text(s.baseDir, style = MaterialTheme.typography.bodySmall)
                        Text(
                            "${s.bookmarks} bookmark(s) · ${s.tags} tag(s) · " +
                                "${s.folders} folder(s) · ${s.rules} rule(s)",
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                        if (s.activeProfile.isNotEmpty()) {
                            CountChip("Profile: ${s.activeProfile}")
                        }
                        OutlinedButton(
                            onClick = onOpenProfiles,
                            modifier = Modifier.padding(top = 8.dp),
                        ) {
                            Text("Profiles")
                        }
                        OutlinedButton(
                            onClick = onOpenLibrary,
                            modifier = Modifier.padding(top = 8.dp),
                        ) {
                            Text("Library")
                        }
                        Text(
                            s.maintenanceStatus,
                            style = MaterialTheme.typography.labelSmall,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }
                Text(
                    "Archive backend",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Text(
                    "On Android only the native snapshot is available; " +
                        "empty resolves to the native default.",
                    style = MaterialTheme.typography.bodySmall,
                )
                ElevatedCard(modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    Column(modifier = Modifier.padding(8.dp)) {
                        backends.forEach { b ->
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                TextButton(
                                    onClick = { selectBackend(b) },
                                    enabled = !saving && s.archiveBackend != b,
                                ) {
                                    Text(if (s.archiveBackend == b) "● $b" else b)
                                }
                            }
                        }
                    }
                }
                if (saving) {
                    Text(
                        "Saving...",
                        style = MaterialTheme.typography.labelMedium,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            }
        }
        }
    }
}

@Composable
fun SavedScreen(api: LiberApi, id: Int, kind: String, label: String, onBack: () -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var html by remember { mutableStateOf<String?>(null) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        error = null
        runApi(main, { api.content(id, kind) }) { res ->
            loading = false
            res.onSuccess { html = it }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(id, kind) { load() }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = label, onBack = onBack)
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(16.dp))
            error != null -> Column(modifier = Modifier.padding(horizontal = 16.dp)) {
                ErrorBlock(error = error ?: "", onRetry = { load() })
            }
            else -> androidx.compose.ui.viewinterop.AndroidView(
                factory = { ctx ->
                    android.webkit.WebView(ctx).apply {
                        settings.javaScriptEnabled = false
                        settings.blockNetworkLoads = true
                        settings.blockNetworkImage = true
                    }
                },
                update = { view ->
                    view.loadDataWithBaseURL(null, html.orEmpty(), "text/html", "utf-8", null)
                },
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

@Composable
fun ProfilesScreen(api: LiberApi, onBack: () -> Unit, onSwitched: () -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var profiles by remember { mutableStateOf(listOf<ApiProfile>()) }
    var mutating by remember { mutableStateOf(false) }
    var name by remember { mutableStateOf("") }
    var deleteTarget by remember { mutableStateOf<ApiProfile?>(null) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        error = null
        runApi(main, { api.profiles() }) { res ->
            loading = false
            res.onSuccess { profiles = it.profiles }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(Unit) { load() }

    fun submitSwitch(target: ApiProfile) {
        mutating = true
        error = null
        runApi(main, { api.switchProfile(target.name) }) { res ->
            mutating = false
            res.onSuccess { onSwitched() }
                .onFailure { error = it.message ?: "switch failed" }
        }
    }

    fun submitCreate() {
        val n = name
        mutating = true
        error = null
        runApi(main, { api.switchProfile(n) }) { res ->
            mutating = false
            res.onSuccess {
                name = ""
                load()
            }.onFailure {
                error = it.message ?: "create failed"
            }
        }
    }

    fun submitDelete() {
        val target = deleteTarget ?: return
        mutating = true
        error = null
        runApi(main, { api.deleteProfile(target.name) }) { res ->
            mutating = false
            res.onSuccess {
                deleteTarget = null
                load()
            }.onFailure {
                deleteTarget = null
                error = it.message ?: "delete failed"
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Profiles", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && profiles.isEmpty() -> ErrorBlock(error = error ?: "", onRetry = { load() })
            else -> {
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Text(
                    "Switching changes the active collection everywhere.",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 8.dp),
                )
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(profiles, key = { it.name }) { p ->
                        ElevatedCard(modifier = Modifier.fillMaxWidth()) {
                            Row(
                                modifier = Modifier.fillMaxWidth().padding(12.dp),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        (if (p.active) "● " else "") + p.name,
                                        style = MaterialTheme.typography.titleMedium,
                                    )
                                    Text(p.path, style = MaterialTheme.typography.bodySmall)
                                }
                                if (!p.active) {
                                    TextButton(
                                        onClick = { submitSwitch(p) },
                                        enabled = !mutating,
                                    ) {
                                        Text("Switch")
                                    }
                                    if (!p.default) {
                                        IconButton(
                                            onClick = { deleteTarget = p },
                                            enabled = !mutating,
                                        ) {
                                            Icon(Icons.Filled.Delete, contentDescription = "Delete")
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Text(
                    "New profile",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    placeholder = { Text("Name (switches to it)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                Button(
                    onClick = { submitCreate() },
                    enabled = !mutating && name.isNotBlank(),
                    modifier = Modifier.padding(top = 4.dp),
                ) {
                    Text(if (mutating) "Saving..." else "Create and switch")
                }
            }
        }
        }
    }

    deleteTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Remove profile?") },
            text = { Text("Stop tracking \"${target.name}\"? Its folder and bookmarks stay on disk.") },
            confirmButton = {
                TextButton(onClick = { submitDelete() }, enabled = !mutating) {
                    Text(if (mutating) "Removing..." else "Remove")
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteTarget = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
fun HistoryScreen(api: LiberApi, onBack: () -> Unit, onOpenDetail: (Int) -> Unit) {
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var rows by remember { mutableStateOf(listOf<ApiHistoryRow>()) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        error = null
        runApi(main, { api.history() }) { res ->
            loading = false
            res.onSuccess { rows = it }
                .onFailure { error = it.message ?: "request failed" }
        }
    }

    androidx.compose.runtime.LaunchedEffect(Unit) { load() }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "History", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && rows.isEmpty() -> ErrorBlock(error = error ?: "", onRetry = { load() })
            else -> {
                if (rows.isEmpty()) {
                    Text(
                        "No open history yet. Opening a bookmark records it here.",
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(rows, key = { it.id }) { r ->
                        ElevatedCard(
                            onClick = { onOpenDetail(r.id) },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Column(modifier = Modifier.padding(12.dp)) {
                                Text(r.title, style = MaterialTheme.typography.titleMedium)
                                Text(r.url, style = MaterialTheme.typography.bodySmall)
                                CountChip(
                                    "opened ${r.openCount}x" +
                                        (r.lastOpenedAt.take(10).takeIf { it.isNotEmpty() }?.let { " · $it" } ?: ""),
                                )
                            }
                        }
                    }
                }
            }
        }
        }
    }
}

@Composable
fun LibraryScreen(api: LiberApi, onBack: () -> Unit) {
    val context = LocalContext.current
    var error by remember { mutableStateOf<String?>(null) }
    var result by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var push by remember { mutableStateOf(false) }
    var rxMerge by remember { mutableStateOf(true) }
    var rxPrune by remember { mutableStateOf(false) }
    var rxCompact by remember { mutableStateOf(false) }
    var rxPruneJournal by remember { mutableStateOf(false) }
    var showReindexConfirm by remember { mutableStateOf(false) }
    val main = Handler(Looper.getMainLooper())

    val picker = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        busy = true
        error = null
        result = null
        runApi(
            main,
            {
                val bytes = context.contentResolver.openInputStream(uri)?.readBytes()
                    ?: throw java.io.IOException("cannot read file")
                api.importLibrary(bytes.toString(Charsets.UTF_8))
            },
        ) { res ->
            busy = false
            res.onSuccess { out ->
                result = "Imported ${out.imported} bookmark(s)" +
                    (if (out.skippedDup > 0) ", skipped ${out.skippedDup} duplicate(s)" else "") +
                    (if (out.skippedBad > 0) ", skipped ${out.skippedBad} bad" else "") +
                    (if (out.warnings.isNotEmpty()) "\n" + out.warnings.joinToString("\n") else "")
            }.onFailure {
                error = it.message ?: "import failed"
            }
        }
    }

    fun submitExport() {
        busy = true
        error = null
        result = null
        runApi(main, { api.exportLibrary() }) { res ->
            busy = false
            res.onSuccess { bytes ->
                try {
                    val dir = java.io.File(context.cacheDir, "liber-library").apply { mkdirs() }
                    val file = java.io.File(dir, "liber-bookmarks.html")
                    file.writeBytes(bytes)
                    val uri = androidx.core.content.FileProvider.getUriForFile(
                        context, "bkm.liber.fileprovider", file,
                    )
                    val send = Intent(Intent.ACTION_SEND).apply {
                        type = "text/html"
                        putExtra(Intent.EXTRA_STREAM, uri)
                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    context.startActivity(Intent.createChooser(send, "Export bookmarks"))
                } catch (e: Exception) {
                    error = e.message ?: "share failed"
                }
            }.onFailure {
                error = it.message ?: "export failed"
            }
        }
    }

    fun submitSite() {
        busy = true
        error = null
        result = null
        runApi(main, { api.exportSite() }) { res ->
            busy = false
            res.onSuccess { (path, count) ->
                result = "Exported $count bookmark(s) to $path"
            }.onFailure {
                error = it.message ?: "export failed"
            }
        }
    }

    fun submitSync() {
        val doPush = push
        busy = true
        error = null
        result = null
        runApi(main, { api.syncNow(doPush) }) { res ->
            busy = false
            res.onSuccess { out ->
                result = (out.output + (out.error.takeIf { it.isNotEmpty() }?.let { "\n$it" } ?: ""))
                    .takeIf { it.isNotBlank() } ?: "Sync done."
            }.onFailure {
                error = it.message ?: "sync failed"
            }
        }
    }

    fun submitReindex() {
        val m = rxMerge
        val p = rxPrune
        val c = rxCompact
        val pj = rxPruneJournal
        busy = true
        error = null
        result = null
        runApi(main, { api.reindex(m, p, c, pj) }) { res ->
            busy = false
            res.onSuccess { out ->
                result = (out.output + (out.error.takeIf { it.isNotEmpty() }?.let { "\n$it" } ?: ""))
                    .takeIf { it.isNotBlank() } ?: "Reindex done."
            }.onFailure {
                error = it.message ?: "reindex failed"
            }
        }
    }

    @Composable
    fun CheckRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Checkbox(checked = checked, onCheckedChange = onChange)
            Text(
                label,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(top = 12.dp),
            )
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LiberTopBar(title = "Library", onBack = onBack)
        Column(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
            if (error != null) {
                Text(
                    text = error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            if (result != null) {
                ElevatedCard(modifier = Modifier.fillMaxWidth().padding(top = 8.dp)) {
                    Text(
                        text = result ?: "",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(12.dp),
                    )
                }
            }
            Text("Import", style = MaterialTheme.typography.titleMedium)
            Text(
                "Import a browser bookmark export. Large files are capped at 1MB per request.",
                style = MaterialTheme.typography.bodySmall,
            )
            Button(
                onClick = { picker.launch(arrayOf("text/html", "text/*")) },
                enabled = !busy,
                modifier = Modifier.padding(top = 4.dp),
            ) {
                Text(if (busy) "Working..." else "Pick file")
            }
            Text(
                "Export",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(top = 16.dp),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { submitExport() }, enabled = !busy) {
                    Text("Share export")
                }
                OutlinedButton(onClick = { submitSite() }, enabled = !busy) {
                    Text("Static site")
                }
            }
            Text(
                "Sync",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(top = 16.dp),
            )
            CheckRow("Push after commit", push) { push = it }
            Button(onClick = { submitSync() }, enabled = !busy) {
                Text(if (busy) "Working..." else "Run sync")
            }
            Text(
                "Maintenance",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(top = 16.dp),
            )
            CheckRow("Merge conflict copies", rxMerge) { rxMerge = it }
            CheckRow("Prune pending entries", rxPrune) { rxPrune = it }
            CheckRow("Compact ids", rxCompact) { rxCompact = it }
            CheckRow("Prune journal", rxPruneJournal) { rxPruneJournal = it }
            Button(
                onClick = {
                    if (rxPrune || rxCompact) showReindexConfirm = true else submitReindex()
                },
                enabled = !busy,
                modifier = Modifier.padding(top = 4.dp),
            ) {
                Text(if (busy) "Working..." else "Run reindex")
            }
        }
    }

    if (showReindexConfirm) {
        AlertDialog(
            onDismissRequest = { showReindexConfirm = false },
            title = { Text("Run reindex?") },
            text = { Text("Prune drops pending entries and compact renames files. Only do this on a fully synced collection.") },
            confirmButton = {
                TextButton(
                    onClick = {
                        showReindexConfirm = false
                        submitReindex()
                    },
                ) {
                    Text("Continue")
                }
            },
            dismissButton = {
                TextButton(onClick = { showReindexConfirm = false }) {
                    Text("Cancel")
                }
            },
        )
    }
}
