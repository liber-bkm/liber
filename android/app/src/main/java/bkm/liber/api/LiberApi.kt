package bkm.liber.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
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

    fun add(url: String, title: String, confirmed: Boolean): ApiBookmark {
        val payload = JSONObject()
            .put("url", url)
            .put("title", title)
            .put("confirm_dup", confirmed)
            .toString()
        val req = authed(
            Request.Builder()
                .url("$base/api/v1/bookmarks")
                .post(payload.toRequestBody("application/json".toMediaType())),
        ).build()
        client.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (resp.code == 409) {
                throw Duplicate(duplicateOf(body), "possible duplicate")
            }
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
}
