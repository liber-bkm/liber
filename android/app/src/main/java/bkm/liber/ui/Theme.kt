package bkm.liber.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val GruvboxDarkBg = Color(0xFF282828)
private val GruvboxDarkBg1 = Color(0xFF3C3836)
private val GruvboxDarkBg2 = Color(0xFF504945)
private val GruvboxDarkFg = Color(0xFFEBDBB2)
private val GruvboxDarkFgDim = Color(0xFFA89984)
private val GruvboxDarkAqua = Color(0xFF8EC07C)
private val GruvboxDarkAquaDim = Color(0xFF689D6A)
private val GruvboxDarkYellow = Color(0xFFFABD2F)
private val GruvboxDarkOrange = Color(0xFFFE8019)
private val GruvboxDarkRed = Color(0xFFFB4934)
private val GruvboxDarkBlue = Color(0xFF83A598)
private val GruvboxDarkPurple = Color(0xFFD3869B)

private val GruvboxLightBg = Color(0xFFFBF1C7)
private val GruvboxLightBg1 = Color(0xFFEBDBB2)
private val GruvboxLightBg2 = Color(0xFFD5C4A1)
private val GruvboxLightFg = Color(0xFF3C3836)
private val GruvboxLightFgDim = Color(0xFF665C54)
private val GruvboxLightGreen = Color(0xFF79740E)
private val GruvboxLightGreenDim = Color(0xFF98971A)
private val GruvboxLightYellow = Color(0xFFB57614)
private val GruvboxLightOrange = Color(0xFFAF3A03)
private val GruvboxLightRed = Color(0xFFCC241D)
private val GruvboxLightBlue = Color(0xFF458588)
private val GruvboxLightPurple = Color(0xFF8F3F71)

private val GruvboxDarkScheme = darkColorScheme(
    primary = GruvboxDarkAqua,
    onPrimary = GruvboxDarkBg,
    primaryContainer = GruvboxDarkAquaDim,
    onPrimaryContainer = GruvboxDarkBg,
    secondary = GruvboxDarkBlue,
    onSecondary = GruvboxDarkBg,
    secondaryContainer = GruvboxDarkBg2,
    onSecondaryContainer = GruvboxDarkFg,
    tertiary = GruvboxDarkYellow,
    onTertiary = GruvboxDarkBg,
    tertiaryContainer = GruvboxDarkBg2,
    onTertiaryContainer = GruvboxDarkYellow,
    error = GruvboxDarkRed,
    onError = GruvboxDarkBg,
    errorContainer = GruvboxDarkBg2,
    onErrorContainer = GruvboxDarkRed,
    background = GruvboxDarkBg,
    onBackground = GruvboxDarkFg,
    surface = GruvboxDarkBg,
    onSurface = GruvboxDarkFg,
    surfaceVariant = GruvboxDarkBg1,
    onSurfaceVariant = GruvboxDarkFgDim,
    surfaceContainerLowest = GruvboxDarkBg,
    surfaceContainerLow = GruvboxDarkBg,
    surfaceContainer = GruvboxDarkBg1,
    surfaceContainerHigh = GruvboxDarkBg1,
    surfaceContainerHighest = GruvboxDarkBg2,
    outline = GruvboxDarkBg2,
    outlineVariant = GruvboxDarkBg1,
)

private val GruvboxLightScheme = lightColorScheme(
    primary = GruvboxLightGreen,
    onPrimary = GruvboxLightBg,
    primaryContainer = GruvboxLightGreenDim,
    onPrimaryContainer = GruvboxLightBg,
    secondary = GruvboxLightBlue,
    onSecondary = GruvboxLightBg,
    secondaryContainer = GruvboxLightBg1,
    onSecondaryContainer = GruvboxLightFg,
    tertiary = GruvboxLightYellow,
    onTertiary = GruvboxLightBg,
    tertiaryContainer = GruvboxLightBg1,
    onTertiaryContainer = GruvboxLightYellow,
    error = GruvboxLightRed,
    onError = GruvboxLightBg,
    errorContainer = GruvboxLightBg1,
    onErrorContainer = GruvboxLightRed,
    background = GruvboxLightBg,
    onBackground = GruvboxLightFg,
    surface = GruvboxLightBg,
    onSurface = GruvboxLightFg,
    surfaceVariant = GruvboxLightBg1,
    onSurfaceVariant = GruvboxLightFgDim,
    surfaceContainerLowest = GruvboxLightBg,
    surfaceContainerLow = GruvboxLightBg,
    surfaceContainer = GruvboxLightBg1,
    surfaceContainerHigh = GruvboxLightBg1,
    surfaceContainerHighest = GruvboxLightBg2,
    outline = GruvboxLightBg2,
    outlineVariant = GruvboxLightBg1,
)

@Composable
fun LiberTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit,
) {
    val scheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val context = LocalContext.current
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        darkTheme -> GruvboxDarkScheme
        else -> GruvboxLightScheme
    }
    MaterialTheme(colorScheme = scheme, content = content)
}
