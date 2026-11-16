package rs.liber.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ShareParserTest {
  @Test
  fun urlOnly() {
    val link = ShareParser.parse("https://example.com/page", null, null)
    assertEquals("https://example.com/page", link?.url)
    assertNull(link?.title)
    assertNull(link?.description)
  }

  @Test
  fun subjectBecomesTitle() {
    val link = ShareParser.parse("https://example.com/page", "Example page", null)
    assertEquals("https://example.com/page", link?.url)
    assertEquals("Example page", link?.title)
    assertNull(link?.description)
  }

  @Test
  fun titleBeforeUrlOnAnotherLine() {
    val link = ShareParser.parse("Example page\nhttps://example.com/page", null, null)
    assertEquals("https://example.com/page", link?.url)
    assertEquals("Example page", link?.title)
  }

  @Test
  fun leftoverTextBecomesDescriptionWhenTitleIsKnown() {
    val link = ShareParser.parse(
      "Read this later\nhttps://example.com/page",
      "Example page",
      null
    )
    assertEquals("https://example.com/page", link?.url)
    assertEquals("Example page", link?.title)
    assertEquals("Read this later", link?.description)
  }

  @Test
  fun textWithoutUrlBecomesDescription() {
    val link = ShareParser.parse("A thought worth saving", null, null)
    assertNull(link?.url)
    assertNull(link?.description)
    assertEquals("A thought worth saving", link?.title)
  }

  @Test
  fun bareDomainGetsScheme() {
    val link = ShareParser.parse("example.com/page", null, null)
    assertEquals("https://example.com/page", link?.url)
  }

  @Test
  fun trailingPunctuationIsTrimmed() {
    val link = ShareParser.parse("See https://example.com/page).", null, null)
    assertEquals("https://example.com/page", link?.url)
  }

  @Test
  fun htmlShareKeepsTheLinkText() {
    val link = ShareParser.parse(
      """<a href="https://example.com/page">Example page</a>""",
      null,
      null
    )
    assertEquals("https://example.com/page", link?.url)
    assertEquals("Example page", link?.title)
    assertNull(link?.description)
  }

  @Test
  fun htmlEntitiesInUrlAreDecoded() {
    val link = ShareParser.parse(
      """<a href="https://example.com/x?a=1&amp;b=2">Page</a>""",
      null,
      null
    )
    assertEquals("https://example.com/x?a=1&b=2", link?.url)
  }

  @Test
  fun emptyShareIsIgnored() {
    assertNull(ShareParser.parse("   ", null, null))
    assertNull(ShareParser.parse(null, null, null))
  }
}
