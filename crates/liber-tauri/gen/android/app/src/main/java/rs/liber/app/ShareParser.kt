package rs.liber.app

import android.content.Intent
import android.text.Html
import android.text.Spanned

data class SharedLink(
  val url: String? = null,
  val title: String? = null,
  val description: String? = null,
  val text: String? = null,
)

object ShareParser {
  private val URL_PATTERN = Regex("""https?://[^\s<>"']+""", RegexOption.IGNORE_CASE)
  private val BARE_DOMAIN = Regex("""^[A-Za-z0-9][A-Za-z0-9.-]*\.[A-Za-z]{2,}(?:/\S*)?$""")
  private val HTML_TAG = Regex("""</?[A-Za-z][^>]*>""")
  private val WHITESPACE = Regex("""\s+""")

  fun fromIntent(intent: Intent?): SharedLink? {
    if (intent == null || intent.action != Intent.ACTION_SEND) return null
    val type = intent.type ?: return null
    if (!type.startsWith("text/")) return null
    val text = when (val raw = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)) {
      is Spanned -> Html.toHtml(raw, Html.TO_HTML_PARAGRAPH_LINES_CONSECUTIVE)
      else -> raw?.toString()
    } ?: intent.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.text?.toString()
    val subject = intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT)?.toString()
    val title = intent.getCharSequenceExtra(Intent.EXTRA_TITLE)?.toString()
    return parse(text, subject, title)
  }

  fun parse(text: String?, subject: String?, title: String?): SharedLink? {
    val raw = text?.trim().orEmpty()
    val suppliedTitle = subject?.trim().orEmpty().ifBlank { title?.trim().orEmpty() }
    if (raw.isEmpty() && suppliedTitle.isEmpty()) return null

    val match = URL_PATTERN.find(raw)
    val url = match?.value?.let(::trimUrl)
      ?: raw.takeIf { BARE_DOMAIN.matches(it) }?.let { "https://$it" }
    val leftover = when {
      match != null -> stripHtml(raw.replaceRange(match.range, " "))
      url != null -> ""
      else -> stripHtml(raw)
    }

    var finalTitle = suppliedTitle
    var description = leftover
    if (finalTitle.isEmpty() && leftover.isNotEmpty() && !leftover.contains('\n') && leftover.length <= 200) {
      finalTitle = leftover
      description = ""
    }

    return SharedLink(
      url = url,
      title = finalTitle.ifBlank { null },
      description = description.ifBlank { null },
      text = raw.ifBlank { null },
    )
  }

  private fun stripHtml(value: String): String =
    if (!HTML_TAG.containsMatchIn(value)) value.trim()
    else WHITESPACE.replace(HTML_TAG.replace(value, " "), " ").trim()

  private fun trimUrl(value: String): String =
    value.trimEnd('.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'')
      .replace("&amp;", "&")
}
