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
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import bkm.liber.api.ApiBookmark
import bkm.liber.api.ApiCheckRow
import bkm.liber.api.ApiFolder
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
            MaterialTheme {
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
}

@Composable
fun LiberNav(api: LiberApi) {
    var stack by remember { mutableStateOf(listOf<Screen>(Screen.List)) }
    val push = { s: Screen -> stack = stack + s }
    val pop = { if (stack.size > 1) stack = stack.dropLast(1) }
    BackHandler(enabled = stack.size > 1) { pop() }
    when (val top = stack.last()) {
        is Screen.List -> SearchScreen(
            api = api,
            onOpenDetail = { push(Screen.Detail(it)) },
            onOpenAdd = { push(Screen.Add) },
            onOpenTags = { push(Screen.Tags) },
            onOpenCheck = { push(Screen.Check) },
            onOpenSettings = { push(Screen.Settings) },
        )
        is Screen.Detail -> DetailScreen(
            api = api,
            id = top.id,
            onBack = { pop() },
            onOpenEdit = { push(Screen.Edit(top.id)) },
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

@Composable
fun SearchScreen(
    api: LiberApi,
    onOpenDetail: (Int) -> Unit,
    onOpenAdd: () -> Unit,
    onOpenTags: () -> Unit,
    onOpenCheck: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    var query by remember { mutableStateOf("") }
    var loading by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var total by remember { mutableStateOf(0) }
    var results by remember { mutableStateOf(listOf<ApiBookmark>()) }
    val main = Handler(Looper.getMainLooper())

    fun runSearch() {
        val q = query
        loading = true
        error = null
        runApi(main, { api.list(q, scope = "", deep = false, sort = "", page = 1) }) { res ->
            loading = false
            res.onSuccess {
                total = it.total
                results = it.bookmarks
            }.onFailure {
                error = it.message ?: "request failed"
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Search...") },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            Button(onClick = { runSearch() }) {
                Text("Go")
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onOpenAdd) {
                Text("Add")
            }
            Button(onClick = onOpenTags) {
                Text("Tags")
            }
            Button(onClick = onOpenCheck) {
                Text("Check")
            }
            Button(onClick = onOpenSettings) {
                Text("Settings")
            }
        }
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null -> Text(
                text = error ?: "",
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(top = 16.dp),
            )
            else -> {
                Text(
                    text = "$total bookmark(s)",
                    style = MaterialTheme.typography.labelMedium,
                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                )
                if (results.isEmpty()) {
                    Text("No results yet. Search above.")
                }
                LazyColumn {
                    items(results, key = { it.id }) { b ->
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onOpenDetail(b.id) }
                                .padding(vertical = 8.dp),
                        ) {
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
    }
}

@Composable
fun DetailScreen(api: LiberApi, id: Int, onBack: () -> Unit, onOpenEdit: () -> Unit) {
    val context = LocalContext.current
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var bookmark by remember { mutableStateOf<ApiBookmark?>(null) }
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

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
        }
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null -> {
                Text(
                    text = error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Button(onClick = { load() }, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Retry")
                }
            }
            else -> bookmark?.let { b ->
                Text(b.title, style = MaterialTheme.typography.headlineSmall)
                Text(b.url, style = MaterialTheme.typography.bodyMedium)
                if (b.description.isNotEmpty()) {
                    Text(b.description, modifier = Modifier.padding(top = 8.dp))
                }
                if (b.folder.isNotEmpty()) {
                    Text("Folder: ${b.folder}", style = MaterialTheme.typography.labelMedium)
                }
                if (b.tags.isNotEmpty()) {
                    Text("Tags: ${b.tags.joinToString(", ")}", style = MaterialTheme.typography.labelMedium)
                }
                Text(
                    "md: ${if (b.hasMarkdown) "yes" else "no"} · archive: ${if (b.hasArchive) "yes" else "no"}" +
                        " · opened ${b.openCount}x",
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier.padding(top = 8.dp),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(
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
                        Text("Open")
                    }
                    Button(
                        onClick = onOpenEdit,
                        modifier = Modifier.padding(top = 8.dp),
                    ) {
                        Text("Edit")
                    }
                }
            }
        }
    }
}

@Composable
fun AddScreen(api: LiberApi, onBack: () -> Unit, onAdded: () -> Unit) {
    var url by remember { mutableStateOf("") }
    var title by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    var saving by remember { mutableStateOf(false) }
    var pendingDup by remember { mutableStateOf<ApiBookmark?>(null) }
    var pendingTitle by remember { mutableStateOf("") }
    val main = Handler(Looper.getMainLooper())

    fun submit(confirmed: Boolean) {
        saving = true
        error = null
        val u = url
        val t = title
        runApi(main, { api.add(u, t, confirmed) }) { res ->
            saving = false
            res.onSuccess { onAdded() }.onFailure { e ->
                val dup = (e as? LiberApi.Duplicate)?.existing
                if (dup != null && !confirmed) {
                    pendingDup = dup
                    pendingTitle = t
                } else {
                    error = e.message ?: "add failed"
                }
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
        }
        TextField(
            value = url,
            onValueChange = { url = it },
            placeholder = { Text("https://example.com") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        TextField(
            value = title,
            onValueChange = { title = it },
            placeholder = { Text("Title (optional)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        if (error != null) {
            Text(
                text = error ?: "",
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
        Button(
            onClick = { submit(confirmed = false) },
            enabled = !saving && url.isNotBlank(),
            modifier = Modifier.padding(top = 8.dp),
        ) {
            Text(if (saving) "Saving..." else "Add")
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
    var loading by remember { mutableStateOf(true) }
    var loadError by remember { mutableStateOf<String?>(null) }
    var title by remember { mutableStateOf("") }
    var url by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var tags by remember { mutableStateOf("") }
    var folder by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    var saving by remember { mutableStateOf(false) }
    val main = Handler(Looper.getMainLooper())

    fun load() {
        loading = true
        loadError = null
        runApi(main, { api.get(id) }) { res ->
            loading = false
            res.onSuccess { b ->
                title = b.title
                url = b.url
                description = b.description
                tags = b.tags.joinToString(", ")
                folder = b.folder
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
        runApi(main, { api.update(id, t, u, d, tagList, f) }) { res ->
            saving = false
            res.onSuccess { onSaved() }.onFailure {
                error = it.message ?: "save failed"
            }
        }
    }

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
        }
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            loadError != null -> {
                Text(
                    text = loadError ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Button(onClick = { load() }, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Retry")
                }
            }
            else -> {
                TextField(
                    value = title,
                    onValueChange = { title = it },
                    placeholder = { Text("Title") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                TextField(
                    value = url,
                    onValueChange = { url = it },
                    placeholder = { Text("https://example.com") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                TextField(
                    value = description,
                    onValueChange = { description = it },
                    placeholder = { Text("Description (optional)") },
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                TextField(
                    value = tags,
                    onValueChange = { tags = it },
                    placeholder = { Text("Tags, comma separated (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                TextField(
                    value = folder,
                    onValueChange = { folder = it },
                    placeholder = { Text("Folder (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
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

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
            Button(onClick = onOpenFolders) {
                Text("Folders")
            }
            Button(onClick = onOpenRules) {
                Text("Rules")
            }
        }
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && tags.isEmpty() -> {
                Text(
                    text = error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Button(onClick = { load() }, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Retry")
                }
            }
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
                LazyColumn {
                    items(tags, key = { it.name }) { t ->
                        Row(
                            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(t.name, style = MaterialTheme.typography.titleMedium)
                                Text(
                                    "${t.count} bookmark(s)",
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                            TextButton(
                                onClick = {
                                    renameTarget = t
                                    renameValue = t.name
                                },
                                enabled = !mutating,
                            ) {
                                Text("Rename")
                            }
                            TextButton(
                                onClick = {
                                    deleteTarget = t
                                    deleteCount = t.count
                                    submitDelete(confirmed = false)
                                },
                                enabled = !mutating,
                            ) {
                                Text("Delete")
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
                TextField(
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

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
        }
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && folders.isEmpty() -> {
                Text(
                    text = error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Button(onClick = { load() }, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Retry")
                }
            }
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
                LazyColumn {
                    items(folders, key = { it.name }) { f ->
                        Row(
                            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    f.display.ifEmpty { "/" },
                                    style = MaterialTheme.typography.titleMedium,
                                )
                                Text(
                                    "${f.count} bookmark(s)",
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                            TextButton(
                                onClick = {
                                    renameTarget = f
                                    renameValue = f.name
                                },
                                enabled = !mutating && f.name.isNotEmpty(),
                            ) {
                                Text("Rename")
                            }
                            TextButton(
                                onClick = {
                                    deleteTarget = f
                                    deleteCount = f.count
                                    submitDelete(confirmed = false)
                                },
                                enabled = !mutating && f.name.isNotEmpty(),
                            ) {
                                Text("Delete")
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
                TextField(
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

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
        }
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && rules.isEmpty() -> {
                Text(
                    text = error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Button(onClick = { load() }, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Retry")
                }
            }
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
                LazyColumn(modifier = Modifier.weight(1f, fill = false)) {
                    items(rules, key = { it.id }) { r ->
                        Row(
                            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(r.match, style = MaterialTheme.typography.titleMedium)
                                val detail = (listOf(r.folder) + r.tags)
                                    .filter { it.isNotEmpty() }.joinToString(" · ")
                                if (detail.isNotEmpty()) {
                                    Text(detail, style = MaterialTheme.typography.bodySmall)
                                }
                                Text(
                                    "applied to ${r.appliedCount} bookmark(s)",
                                    style = MaterialTheme.typography.labelSmall,
                                )
                            }
                            TextButton(
                                onClick = { submitApply(r.id) },
                                enabled = !mutating,
                            ) {
                                Text("Apply")
                            }
                            TextButton(
                                onClick = {
                                    deleteTarget = r
                                    deleteCount = r.appliedCount
                                    submitDelete(confirmed = false)
                                },
                                enabled = !mutating,
                            ) {
                                Text("Delete")
                            }
                        }
                    }
                }
                Text(
                    text = "Add rule",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                TextField(
                    value = match,
                    onValueChange = { match = it },
                    placeholder = { Text("Match, e.g. host:example.com") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                TextField(
                    value = folder,
                    onValueChange = { folder = it },
                    placeholder = { Text("Folder (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                TextField(
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
                    TextField(
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
    fun CheckRowView(row: ApiCheckRow, actions: @Composable () -> Unit) {
        Column(modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
            Text("[${row.id}] ${row.title}", style = MaterialTheme.typography.titleMedium)
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

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
        }
        TextField(
            value = spec,
            onValueChange = { spec = it },
            placeholder = { Text("ids like 1-100 (empty = all)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        TextField(
            value = stale,
            onValueChange = { stale = it },
            placeholder = { Text("only stale, e.g. 720h (empty = all)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        )
        Button(
            onClick = { runScan() },
            enabled = !scanning && !mutating,
            modifier = Modifier.padding(top = 8.dp),
        ) {
            Text(if (scanning) "Scanning..." else "Run check")
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
            LazyColumn(modifier = Modifier.weight(1f, fill = false)) {
                if (moved.isNotEmpty()) {
                    item {
                        Text("Moved", style = MaterialTheme.typography.titleMedium)
                    }
                    items(moved, key = { it.id }) { row ->
                        CheckRowView(row) {
                            TextButton(
                                onClick = { submitApply(row, "update", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Text("Update URL")
                            }
                            TextButton(
                                onClick = { submitApply(row, "retitle", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Text("URL + title")
                            }
                        }
                    }
                }
                if (dead.isNotEmpty()) {
                    item {
                        Text("Dead", style = MaterialTheme.typography.titleMedium)
                    }
                    items(dead, key = { it.id }) { row ->
                        CheckRowView(row) {
                            TextButton(
                                onClick = { submitApply(row, "delete", confirmed = false) },
                                enabled = !mutating,
                            ) {
                                Text("Delete")
                            }
                            TextButton(
                                onClick = { submitApply(row, "quarantine", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Text("Quarantine")
                            }
                        }
                    }
                }
                if (uncertain.isNotEmpty()) {
                    item {
                        Text("Uncertain", style = MaterialTheme.typography.titleMedium)
                    }
                    items(uncertain, key = { it.id }) { row ->
                        CheckRowView(row) {
                            TextButton(
                                onClick = { submitApply(row, "delete", confirmed = false) },
                                enabled = !mutating,
                            ) {
                                Text("Delete")
                            }
                            TextButton(
                                onClick = { submitApply(row, "quarantine", confirmed = true) },
                                enabled = !mutating,
                            ) {
                                Text("Quarantine")
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
fun SettingsScreen(api: LiberApi, onBack: () -> Unit) {
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

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onBack) {
                Text("Back")
            }
        }
        when {
            loading -> CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            error != null && settings == null -> {
                Text(
                    text = error ?: "",
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Button(onClick = { load() }, modifier = Modifier.padding(top = 8.dp)) {
                    Text("Retry")
                }
            }
            else -> settings?.let { s ->
                if (error != null) {
                    Text(
                        text = error ?: "",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Text("Collection", style = MaterialTheme.typography.titleMedium)
                Text(s.baseDir, style = MaterialTheme.typography.bodySmall)
                Text(
                    "${s.bookmarks} bookmark(s) · ${s.tags} tag(s) · " +
                        "${s.folders} folder(s) · ${s.rules} rule(s)",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp),
                )
                if (s.activeProfile.isNotEmpty()) {
                    Text(
                        "Profile: ${s.activeProfile}",
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
                Text(
                    s.maintenanceStatus,
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier.padding(top = 4.dp),
                )
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
                Column(modifier = Modifier.padding(top = 4.dp)) {
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
