package bkm.liber.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.contentOrNull
import org.json.JSONObject
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit

@Serializable
data class ApiAttachment(
    val name: String = "",
)

@Serializable
data class ApiBookmark(
    val id: Int = 0,
    val url: String = "",
    val title: String = "",
    val description: String = "",
    val tags: List<String> = emptyList(),
    val folder: String = "",
    @SerialName("has_markdown") val hasMarkdown: Boolean = false,
    @SerialName("has_archive") val hasArchive: Boolean = false,
    val attachments: List<ApiAttachment> = emptyList(),
    @SerialName("open_count") val openCount: Int = 0,
)

@Serializable
data class ApiTag(
    val name: String = "",
    val count: Int = 0,
)

@Serializable
data class ApiTagsResponse(
    val tags: List<ApiTag> = emptyList(),
)

@Serializable
data class ApiFolder(
    val name: String = "",
    val display: String = "",
    val count: Int = 0,
)

@Serializable
data class ApiFoldersResponse(
    val folders: List<ApiFolder> = emptyList(),
)

@Serializable
data class ApiRule(
    val id: Int = 0,
    val match: String = "",
    val folder: String = "",
    val tags: List<String> = emptyList(),
    @SerialName("applied_count") val appliedCount: Int = 0,
)

@Serializable
data class ApiRulesResponse(
    val rules: List<ApiRule> = emptyList(),
)

@Serializable
data class ApiSuggestion(
    val host: String = "",
    val folder: String = "",
    val count: Int = 0,
)

@Serializable
data class ApiSuggestionsResponse(
    val min: Int = 3,
    val suggestions: List<ApiSuggestion> = emptyList(),
)

@Serializable
data class ApiProfile(
    val name: String = "",
    val path: String = "",
    val active: Boolean = false,
    val default: Boolean = false,
)

@Serializable
data class ApiProfilesResponse(
    val active: String = "default",
    val profiles: List<ApiProfile> = emptyList(),
)

@Serializable
data class ApiHistoryRow(
    val id: Int = 0,
    val title: String = "",
    val url: String = "",
    @SerialName("open_count") val openCount: Int = 0,
    @SerialName("last_opened_at") val lastOpenedAt: String = "",
)

@Serializable
data class ApiHistoryResponse(
    val history: List<ApiHistoryRow> = emptyList(),
)

@Serializable
data class ApiImportResult(
    val imported: Int = 0,
    @SerialName("skipped_dup") val skippedDup: Int = 0,
    @SerialName("skipped_bad") val skippedBad: Int = 0,
    val warnings: List<String> = emptyList(),
)

@Serializable
data class ApiCommandResult(
    val output: String = "",
    val error: String = "",
)

@Serializable
data class ApiCheckRow(
    val id: Int = 0,
    val title: String = "",
    val url: String = "",
    val detail: String = "",
    val target: String = "",
    val status: String = "",
)

@Serializable
data class ApiCheckResult(
    val ok: Int = 0,
    val checked: Int = 0,
    val fresh: Int = 0,
    val missing: List<Int> = emptyList(),
    val moved: List<ApiCheckRow> = emptyList(),
    val dead: List<ApiCheckRow> = emptyList(),
    val uncertain: List<ApiCheckRow> = emptyList(),
)

@Serializable
data class ApiSettings(
    @SerialName("base_dir") val baseDir: String = "",
    @SerialName("active_profile") val activeProfile: String = "",
    @SerialName("archive_backend") val archiveBackend: String = "",
    val bookmarks: Int = 0,
    val tags: Int = 0,
    val folders: Int = 0,
    val rules: Int = 0,
    @SerialName("maintenance_status") val maintenanceStatus: String = "",
)

@Serializable
data class ApiListResponse(
    val total: Int = 0,
    val page: Int = 1,
    @SerialName("total_pages") val totalPages: Int = 1,
    val bookmarks: List<ApiBookmark> = emptyList(),
)

class LiberApi(baseUrl: String, token: String) {
    private val base = baseUrl.trimEnd('/')
    private val auth = bearerValue(token.trim())
    private val client = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()
    private val json = Json { ignoreUnknownKeys = true }

    companion object {
        fun updatePayload(
            title: String?,
            url: String?,
            description: String?,
            tags: List<String>?,
            folder: String?,
            markdownText: String? = null,
            archive: Boolean = false,
        ): String {
            return buildJsonObject {
                if (title != null) put("title", JsonPrimitive(title))
                if (url != null) put("url", JsonPrimitive(url))
                if (description != null) put("description", JsonPrimitive(description))
                if (tags != null) put("tags", JsonArray(tags.map { JsonPrimitive(it) }))
                if (folder != null) put("folder", JsonPrimitive(folder))
                if (markdownText != null) put("markdown_text", JsonPrimitive(markdownText))
                if (archive) put("archive", JsonPrimitive(true))
            }.toString()
        }

        fun bearerValue(token: String): String {
            if (token.isEmpty()) return ""
            val mac = javax.crypto.Mac.getInstance("HmacSHA256")
            mac.init(
                javax.crypto.spec.SecretKeySpec(
                    token.toByteArray(Charsets.UTF_8),
                    "HmacSHA256",
                ),
            )
            return mac.doFinal("liber-bearer-v1".toByteArray(Charsets.UTF_8))
                .joinToString("") { "%02x".format(it) }
        }
    }

    fun listUrl(
        q: String,
        scope: String,
        deep: Boolean,
        sort: String,
        page: Int,
    ): String {
        val url = "$base/api/v1/bookmarks".toHttpUrlOrNull()
            ?: throw IOException("bad base url: $base")
        val b = url.newBuilder()
        if (q.isNotEmpty()) b.addQueryParameter("q", q)
        for (c in scope) b.addQueryParameter("scope", c.toString())
        if (deep) b.addQueryParameter("deep", "1")
        if (sort.isNotEmpty()) b.addQueryParameter("sort", sort)
        if (page > 1) b.addQueryParameter("page", page.toString())
        return b.build().toString()
    }

    fun list(
        q: String,
        scope: String,
        deep: Boolean,
        sort: String,
        page: Int,
    ): ApiListResponse {
        val req = authed(Request.Builder().url(listUrl(q, scope, deep, sort, page))).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiListResponse.serializer(), body)
        }
    }

    fun get(id: Int): ApiBookmark {
        val req = authed(Request.Builder().url("$base/api/v1/bookmarks/$id")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such bookmark")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiBookmark.serializer(), body)
        }
    }

    fun add(url: String, title: String, confirmed: Boolean, markdown: Boolean = false, archive: Boolean = false): ApiBookmark {
        val payload = JSONObject()
            .put("url", url)
            .put("title", title)
            .put("confirm_dup", confirmed)
            .put("markdown", markdown)
            .put("archive", archive)
            .toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/bookmarks")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        // Archiving fetches the page synchronously and can take a while.
        slowClient.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 409) {
                throw Duplicate(duplicateOf(body), "possible duplicate")
            }
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiBookmark.serializer(), body)
        }
    }

    fun update(
        id: Int,
        title: String?,
        url: String?,
        description: String?,
        tags: List<String>?,
        folder: String?,
        markdownText: String? = null,
        archive: Boolean = false,
    ): ApiBookmark {
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/bookmarks/$id")
                .put(updatePayload(title, url, description, tags, folder, markdownText, archive).toRequestBody("application/json".toMediaType())),
        ).build()
        // Archiving fetches the page synchronously and can take a while.
        slowClient.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such bookmark")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiBookmark.serializer(), body)
        }
    }

    fun open(id: Int): String {
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/bookmarks/$id/open")
                .post(ByteArray(0).toRequestBody(null)),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such bookmark")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val url = json.parseToJsonElement(body).jsonObject["url"]
                ?.jsonPrimitive?.contentOrNull
            return url ?: throw IOException("bad open response")
        }
    }

    fun delete(id: Int, confirmed: Boolean): Int {
        val url = "$base/api/v1/bookmarks/$id" +
            if (confirmed) "?confirm=true" else ""
        val req = authed(Request.Builder().url(url).delete()).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such bookmark")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            if (obj["confirm_required"]?.jsonPrimitive?.contentOrNull == "true") {
                throw ConfirmRequired(1)
            }
            return obj["deleted"]?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad delete response")
        }
    }

    fun content(id: Int, kind: String): String {
        val req = authed(Request.Builder().url("$base/api/v1/bookmarks/$id/$kind")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no saved $kind")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return body
        }
    }

    fun downloadAttachment(id: Int, n: Int): ByteArray {
        val req = authed(Request.Builder().url("$base/api/v1/bookmarks/$id/attachments/$n")).build()
        client.newCall(req).execute().use { resp ->
            if (resp.code == 404) throw IOException("no such attachment")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}")
            return resp.body?.bytes() ?: throw IOException("empty attachment")
        }
    }

    fun rawMarkdown(id: Int): String {
        val req = authed(Request.Builder().url("$base/api/v1/bookmarks/$id/markdown?raw=1")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no saved markdown")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return body
        }
    }

    fun uploadAttachment(id: Int, filename: String, bytes: ByteArray): String {
        val payload = buildJsonObject {
            put("filename", JsonPrimitive(filename))
            put("content_base64", JsonPrimitive(android.util.Base64.encodeToString(bytes, android.util.Base64.NO_WRAP)))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/bookmarks/$id/attachments")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such bookmark")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["name"]
                ?.jsonPrimitive?.contentOrNull
                ?: throw IOException("bad upload response")
        }
    }

    fun deleteAttachment(id: Int, n: Int): String {
        val req = authed(Request.Builder().url("$base/api/v1/bookmarks/$id/attachments/$n").delete()).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such attachment")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["deleted"]
                ?.jsonPrimitive?.contentOrNull
                ?: throw IOException("bad delete response")
        }
    }

    fun tags(): List<ApiTag> {
        val req = authed(Request.Builder().url("$base/api/v1/tags")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiTagsResponse.serializer(), body).tags
        }
    }

    fun renameTag(old: String, new: String): Int {
        val payload = buildJsonObject {
            put("old", JsonPrimitive(old))
            put("new", JsonPrimitive(new))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/tags/rename")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no bookmarks have that tag")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["renamed"]
                ?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad rename response")
        }
    }

    fun deleteTag(tag: String, confirmed: Boolean): Int {
        val payload = buildJsonObject {
            put("tag", JsonPrimitive(tag))
            put("confirm", JsonPrimitive(confirmed))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/tags/delete")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no bookmarks have that tag")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            if (obj["confirm_required"]?.jsonPrimitive?.contentOrNull == "true") {
                throw ConfirmRequired(
                    obj["count"]?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: 0,
                )
            }
            return obj["deleted"]?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad delete response")
        }
    }

    fun folders(): List<ApiFolder> {
        val req = authed(Request.Builder().url("$base/api/v1/folders")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiFoldersResponse.serializer(), body).folders
        }
    }

    fun renameFolder(old: String, new: String): Int {
        val payload = buildJsonObject {
            put("old", JsonPrimitive(old))
            put("new", JsonPrimitive(new))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/folders/rename")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no bookmarks in that folder")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["moved"]
                ?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad rename response")
        }
    }

    fun deleteFolder(folder: String, confirmed: Boolean): Int {
        val payload = buildJsonObject {
            put("folder", JsonPrimitive(folder))
            put("confirm", JsonPrimitive(confirmed))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/folders/delete")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no bookmarks in that folder")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            if (obj["confirm_required"]?.jsonPrimitive?.contentOrNull == "true") {
                throw ConfirmRequired(
                    obj["count"]?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: 0,
                )
            }
            return obj["moved"]?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad delete response")
        }
    }

    fun rules(): List<ApiRule> {
        val req = authed(Request.Builder().url("$base/api/v1/rules")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiRulesResponse.serializer(), body).rules
        }
    }

    fun createRule(match: String, folder: String, tags: List<String>): ApiRule {
        val payload = buildJsonObject {
            put("match", JsonPrimitive(match))
            put("folder", JsonPrimitive(folder))
            put("tags", JsonArray(tags.map { JsonPrimitive(it) }))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/rules")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiRule.serializer(), body)
        }
    }

    fun deleteRule(id: Int, confirmed: Boolean): ApiRule? {
        val url = "$base/api/v1/rules/$id" +
            if (confirmed) "?confirm=true" else ""
        val req = authed(Request.Builder().url(url).delete()).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such rule")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            if (obj["confirm_required"]?.jsonPrimitive?.contentOrNull == "true") {
                throw ConfirmRequired(
                    obj["rule"]?.jsonObject?.get("applied_count")
                        ?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: 0,
                )
            }
            obj["deleted"]?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad delete response")
            return null
        }
    }

    fun applyRules(id: Int?): Int {
        val payload = buildJsonObject {
            if (id != null) put("id", JsonPrimitive(id))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/rules/apply")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such rule")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["applied"]
                ?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad apply response")
        }
    }

    fun suggestions(min: Int): List<ApiSuggestion> {
        val req = authed(
            Request.Builder().url("$base/api/v1/rules/suggestions?min=$min"),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiSuggestionsResponse.serializer(), body).suggestions
        }
    }

    fun learnAll(min: Int): Int {
        val payload = buildJsonObject {
            put("min", JsonPrimitive(min))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/rules/learn")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["created"]
                ?.jsonArray?.size
                ?: throw IOException("bad learn response")
        }
    }

    fun editRule(
        id: Int,
        match: String?,
        folder: String?,
        tags: List<String>?,
        reapply: Boolean,
    ): Pair<ApiRule, Int> {
        val payload = buildJsonObject {
            if (match != null) put("match", JsonPrimitive(match))
            if (folder != null) put("folder", JsonPrimitive(folder))
            if (tags != null) put("tags", JsonArray(tags.map { JsonPrimitive(it) }))
            put("reapply", JsonPrimitive(reapply))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/rules/$id")
                .put(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no such rule")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            val rule = json.decodeFromJsonElement(ApiRule.serializer(), obj)
            val reapplied = obj["reapplied"]?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: 0
            return rule to reapplied
        }
    }

    fun profiles(): ApiProfilesResponse {
        val req = authed(Request.Builder().url("$base/api/v1/profiles")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiProfilesResponse.serializer(), body)
        }
    }

    fun switchProfile(name: String): String {
        val payload = buildJsonObject {
            put("name", JsonPrimitive(name))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/profiles/switch")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["result"]
                ?.jsonPrimitive?.contentOrNull
                ?: throw IOException("bad switch response")
        }
    }

    fun deleteProfile(name: String): String {
        val payload = buildJsonObject {
            put("name", JsonPrimitive(name))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/profiles/delete")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["result"]
                ?.jsonPrimitive?.contentOrNull
                ?: throw IOException("bad delete response")
        }
    }

    fun history(): List<ApiHistoryRow> {
        val req = authed(Request.Builder().url("$base/api/v1/history")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiHistoryResponse.serializer(), body).history
        }
    }

    private fun bulkPayload(ids: Set<Int>, action: String, tags: List<String>?, folder: String?, confirmed: Boolean): String {
        return buildJsonObject {
            put("ids", JsonArray(ids.map { JsonPrimitive(it) }))
            put("action", JsonPrimitive(action))
            if (tags != null) put("tags", JsonArray(tags.map { JsonPrimitive(it) }))
            if (folder != null) put("folder", JsonPrimitive(folder))
            put("confirm", JsonPrimitive(confirmed))
        }.toString()
    }

    private fun postBulk(ids: Set<Int>, action: String, tags: List<String>?, folder: String?, confirmed: Boolean): Int {
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/bulk")
                .post(bulkPayload(ids, action, tags, folder, confirmed).toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("no matching bookmarks")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            if (obj["confirm_required"]?.jsonPrimitive?.contentOrNull == "true") {
                throw ConfirmRequired(
                    obj["count"]?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: 0,
                )
            }
            return obj["count"]?.jsonPrimitive?.contentOrNull?.toIntOrNull()
                ?: throw IOException("bad bulk response")
        }
    }

    fun bulkDelete(ids: Set<Int>, confirmed: Boolean): Int {
        return postBulk(ids, "delete", null, null, confirmed)
    }

    fun bulkTags(ids: Set<Int>, tags: List<String>): Int {
        return postBulk(ids, "tags", tags, null, true)
    }

    fun bulkFolder(ids: Set<Int>, folder: String): Int {
        return postBulk(ids, "folder", null, folder, true)
    }

    private val slowClient = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(10, TimeUnit.MINUTES)
        .build()

    fun checkRun(spec: String, workers: Int, stale: String): ApiCheckResult {
        val payload = buildJsonObject {
            put("spec", JsonPrimitive(spec))
            if (workers > 0) put("workers", JsonPrimitive(workers))
            put("stale", JsonPrimitive(stale))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/check/run")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        slowClient.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiCheckResult.serializer(), body)
        }
    }

    fun checkApply(id: Int, action: String, target: String, confirmed: Boolean): String {
        val payload = buildJsonObject {
            put("id", JsonPrimitive(id))
            put("action", JsonPrimitive(action))
            put("target", JsonPrimitive(target))
            put("confirm", JsonPrimitive(confirmed))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/check/apply")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 404) throw IOException("bookmark already gone")
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            if (obj["confirm_required"]?.jsonPrimitive?.contentOrNull == "true") {
                throw ConfirmRequired(
                    obj["count"]?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: 0,
                )
            }
            return obj["result"]?.jsonPrimitive?.contentOrNull
                ?: throw IOException("bad apply response")
        }
    }

    fun settings(): ApiSettings {
        val req = authed(Request.Builder().url("$base/api/v1/settings")).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiSettings.serializer(), body)
        }
    }

    fun setBackend(backend: String): String {
        val payload = buildJsonObject {
            put("archive_backend", JsonPrimitive(backend))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/settings")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.parseToJsonElement(body).jsonObject["archive_backend"]
                ?.jsonPrimitive?.contentOrNull
                ?: throw IOException("bad settings response")
        }
    }

    fun importLibrary(content: String): ApiImportResult {
        val payload = buildJsonObject {
            put("content", JsonPrimitive(content))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/library/import")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiImportResult.serializer(), body)
        }
    }

    fun exportLibrary(): ByteArray {
        val req = authed(Request.Builder().url("$base/api/v1/library/export")).build()
        client.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}")
            return resp.body?.bytes() ?: throw IOException("empty export")
        }
    }

    fun exportSite(): Pair<String, Int> {
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/library/site")
                .post("{}".toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            val obj = json.parseToJsonElement(body).jsonObject
            val path = obj["path"]?.jsonPrimitive?.contentOrNull
                ?: throw IOException("bad site response")
            val count = obj["count"]?.jsonPrimitive?.contentOrNull?.toIntOrNull() ?: 0
            return path to count
        }
    }

    fun syncNow(push: Boolean): ApiCommandResult {
        val payload = buildJsonObject {
            put("push", JsonPrimitive(push))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/sync")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiCommandResult.serializer(), body)
        }
    }

    fun reindex(merge: Boolean, prune: Boolean, compact: Boolean, pruneJournal: Boolean): ApiCommandResult {
        val payload = buildJsonObject {
            put("merge", JsonPrimitive(merge))
            put("prune", JsonPrimitive(prune))
            put("compact", JsonPrimitive(compact))
            put("prune_journal", JsonPrimitive(pruneJournal))
        }.toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/reindex")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}: $body")
            return json.decodeFromString(ApiCommandResult.serializer(), body)
        }
    }

    private fun authed(b: Request.Builder): Request.Builder {
        if (auth.isNotEmpty()) b.header("Authorization", "Bearer $auth")
        return b
    }

    fun duplicateOf(body: String): ApiBookmark? {
        return try {
            val obj = json.parseToJsonElement(body).jsonObject
            obj["duplicate"]?.let { json.decodeFromJsonElement(ApiBookmark.serializer(), it) }
        } catch (_: Exception) {
            null
        }
    }

    class Duplicate(val existing: ApiBookmark?, message: String) : IOException(message)

    class ConfirmRequired(val count: Int) : IOException("confirm required") {
        constructor() : this(0)
    }
}
