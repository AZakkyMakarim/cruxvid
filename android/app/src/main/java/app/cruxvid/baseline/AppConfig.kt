package app.cruxvid.baseline

/** Swappable seams for E2E: Gemini base URL and Source resolver. Fakes only when built with -PtestMode=true. */
object AppConfig {
    val geminiBaseUrl: String =
        if (BuildConfig.TEST_MODE) "http://10.0.2.2:8787" else "https://generativelanguage.googleapis.com"
    val sourceResolver: String = if (BuildConfig.TEST_MODE) "fake" else "yt-dlp"
}
