package bkm.liber.api

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ApiParsingTest {

    private val json = Json { ignoreUnknownKeys = true }

    private val listFixture = """
        {"total":2,"page":1,"total_pages":1,"bookmarks":[
        {"id":1,"url":"https://a.com/1","title":"alpha","description":"d",
         "tags":["x"],"folder":"docs","created_at":"2026-09-23T00:00:00Z",
         "updated_at":"2026-09-23T00:00:01Z","has_markdown":true,"has_archive":false,
         "attachments":[{"name":"paper.pdf"}],"open_count":3,
         "extra_future_field":"ignored"},
        {"id":2,"url":"https://b.com/2","title":"beta"}
        ]}
    """.trimIndent()

    @Test
    fun parsesListResponse() {
        val out = json.decodeFromString(ApiListResponse.serializer(), listFixture)
        assertEquals(2, out.total)
        assertEquals(2, out.bookmarks.size)
        val first = out.bookmarks[0]
        assertEquals(1, first.id)
        assertEquals("alpha", first.title)
        assertEquals(listOf("x"), first.tags)
        assertEquals("docs", first.folder)
        assertTrue(first.hasMarkdown)
        assertEquals(listOf(ApiAttachment("paper.pdf")), first.attachments)
        val second = out.bookmarks[1]
        assertEquals("beta", second.title)
        assertEquals(emptyList<String>(), second.tags)
    }

    @Test
    fun bearerDerivationMatchesServer() {
        // Must equal hex(HMAC-SHA256("s3cret", "liber-bearer-v1")) as computed
        // by Go's authMAC; verified against the documented recipe.
        val mac = javax.crypto.Mac.getInstance("HmacSHA256")
        mac.init(javax.crypto.spec.SecretKeySpec("s3cret".toByteArray(), "HmacSHA256"))
        val expected = mac.doFinal("liber-bearer-v1".toByteArray())
            .joinToString("") { "%02x".format(it) }
        assertEquals(expected, LiberApi.bearerValue("s3cret"))
        assertEquals("", LiberApi.bearerValue(""))
    }

    @Test
    fun duplicateOfParses() {
        val api = LiberApi("http://x/", "")
        val dup = api.duplicateOf(
            """{"error":"possible duplicate","duplicate":{"id":7,"url":"https://d.com","title":"Dee"},"hint":"x"}""",
        )
        assertEquals(7, dup?.id)
        assertEquals("Dee", dup?.title)
        assertEquals(null, api.duplicateOf("not json"))
    }

    @Test
    fun updatePayloadOmitsAbsentFields() {
        val partial = LiberApi.updatePayload("T", null, null, null, null)
        assertTrue(partial.contains("\"title\":\"T\""))
        assertTrue(!partial.contains("\"url\""))
        assertTrue(!partial.contains("\"tags\""))
        val full = LiberApi.updatePayload("T", "https://e.com", "d", listOf("a", "b"), "f")
        assertTrue(full.contains("\"url\":\"https://e.com\""))
        assertTrue(full.contains("\"description\":\"d\""))
        assertTrue(full.contains("\"folder\":\"f\""))
        assertTrue(full.contains("\"tags\":[\"a\",\"b\"]"))
    }

    @Test
    fun parsesTagsResponse() {
        val out = json.decodeFromString(
            ApiTagsResponse.serializer(),
            """{"tags":[{"name":"x","count":3},{"name":"y","count":1}],"extra":true}""",
        )
        assertEquals(2, out.tags.size)
        assertEquals(ApiTag("x", 3), out.tags[0])
        assertEquals(ApiTag("y", 1), out.tags[1])
        val empty = json.decodeFromString(ApiTagsResponse.serializer(), """{"tags":[]}""")
        assertEquals(0, empty.tags.size)
    }

    @Test
    fun parsesFoldersResponse() {
        val out = json.decodeFromString(
            ApiFoldersResponse.serializer(),
            """{"folders":[{"name":"docs","display":"docs","count":3},{"name":"","display":"/","count":1}]}""",
        )
        assertEquals(2, out.folders.size)
        assertEquals(ApiFolder("docs", "docs", 3), out.folders[0])
        assertEquals(ApiFolder("", "/", 1), out.folders[1])
    }

    @Test
    fun listUrlParams() {
        val api = LiberApi("http://127.0.0.1:8080/", "s3cret")
        val url = api.listUrl("hello world", "nt", deep = true, sort = "newest", page = 2)
        assertTrue(url.startsWith("http://127.0.0.1:8080/api/v1/bookmarks?"))
        assertTrue(url.contains("q=hello%20world"))
        assertTrue(url.contains("scope=n"))
        assertTrue(url.contains("scope=t"))
        assertTrue(url.contains("deep=1"))
        assertTrue(url.contains("sort=newest"))
        assertTrue(url.contains("page=2"))
        val minimal = LiberApi("http://x/", "").listUrl("", "", deep = false, sort = "", page = 1)
        assertEquals("http://x/api/v1/bookmarks", minimal)
    }
}
