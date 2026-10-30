// UI language (Indonesian / English) and theme (Dark / Light / follow the system) of the whole window.
// The choice is saved by the Rust engine (settings.json) so it survives a restart; no choice = follow the system.
import { getUiPrefs, setUiPrefs } from './api.js';
import { DEFAULT_LANGUAGE, LANGUAGES, getLanguage, setLanguage, strings } from './strings.js';

/** Saved choice first; otherwise the first supported language of the system; otherwise Indonesian. */
export function resolveLanguage(saved, systemLanguages = []) {
  if (LANGUAGES.includes(saved)) return saved;
  for (const l of systemLanguages) {
    const base = String(l).toLowerCase().split('-')[0];
    if (LANGUAGES.includes(base)) return base;
  }
  return DEFAULT_LANGUAGE;
}

// The Rust engine reports errors in one fixed language. [pattern, strings.errors key, optional name of a lookup table
// in strings.errors that translates the first captured word].
export const ERROR_RULES = [
  [/^Cannot run ffmpeg \(is it installed\?\): ([\s\S]*)$/, 'ffmpegMissing'],
  [/^Cannot run ffprobe \(is ffmpeg installed\?\): ([\s\S]*)$/, 'ffprobeMissing'],
  [/^Cannot read the audio of this video \(does it have a sound track\?\)\.$/, 'noAudio'],
  [/^This video is too long to send to Gemini in one piece/, 'tooLong'],
  [/^Caption color must look like #RRGGBB\.$/, 'captionColorBad'],
  [/^Caption font name is not valid\.$/, 'captionFontBad'],
  [/^Caption size must be 30-140\.$/, 'captionSizeBad'],
  [/^Caption outline must be 0-12 and position 0-1500\.$/, 'captionOutlineBad'],
  [/^Word not found: ([\s\S]*)$/, 'wordNotFound'],
  [/^The range must lie inside the Source \(0 - ([\d.]+) s\)\.$/, 'rangeOutside'],
  [/^A Clip must be at least (\d+) s long\.$/, 'rangeTooShort'],
  [/^Clip not found: ([\s\S]*)$/, 'clipNotFound'],
  [/^Project not found: ([\s\S]*)$/, 'projectNotFound'],
  [/^Invalid project id: ([\s\S]*)$/, 'projectBadId'],
  [/^Project file is damaged: ([\s\S]*)$/, 'projectDamaged'],
  [/^Layout file is damaged: ([\s\S]*)$/, 'layoutDamaged'],
  [/^Title tidak boleh kosong\.$/, 'titleEmpty'],
  [/^API key tidak boleh kosong\.$/, 'keyEmpty'],
  [/^Penyimpanan key tidak tersedia\.$/, 'keyStoreMissing'],
  [/^Nama model tidak boleh kosong\.$/, 'modelEmpty'],
  [/^Bahasa tidak valid\.$/, 'languageBad'],
  [/^Terjemahan dari Gemini kosong\. Coba lagi\.$/, 'translationEmpty'],
  [/^Export failed: ([\s\S]*)$/, 'exportFailed'],
  [/^Cannot save (caption|translation|view|queue|project|layout|settings|setting|export|the cut): ([\s\S]*)$/, 'saveFailed', 'saveWhat'],
  [/^Cannot write caption file: ([\s\S]*)$/, 'captionFileFailed'],
  [/^Cannot (create output folder|create folder|use that folder|open folder): ([\s\S]*)$/, 'folderFailed', 'folderWhat'],
  [/^Choose a full folder path/, 'folderPathBad'],
  [/^API key Gemini belum diisi\./, 'keyNeeded'],
  [/^Key tidak valid: Gemini menolak/, 'keyRejected'],
  [/^Kuota habis: /, 'quotaUsed'],
  [/^Jaringan bermasalah: /, 'network'],
  [/^Gemini returned HTTP (\d+)\.$/, 'geminiHttp'],
  [/^Gemini response is not JSON: ([\s\S]*)$/, 'geminiNotJson'],
  [/^Gemini response has no text\.$/, 'geminiNoText'],
  [/^Gemini answer does not match the expected shape: ([\s\S]*)$/, 'geminiBadShape'],
  [/^Unknown layout: ([\s\S]*)$/, 'layoutUnknown'],
  [/^No pane (\d+) in layout ([\s\S]*)$/, 'layoutNoPane'],
  [/^Cannot read a preview frame: ([\s\S]*)$/, 'frameFailed'],
  [/^Cannot open the credential store: ([\s\S]*)$/, 'credentialOpen'],
  [/^Cannot read the API key from the credential store: ([\s\S]*)$/, 'credentialRead'],
  [/^Cannot save the API key to the credential store: ([\s\S]*)$/, 'credentialSave'],
  [/^Cannot remove the API key from the credential store: ([\s\S]*)$/, 'credentialRemove'],
  [/^File not found: ([\s\S]*)$/, 'fileNotFound'],
  [/^([\s\S]*) is not a video file\.$/, 'notVideo'],
  [/^Cannot prepare the Clip for speaker detection: ([\s\S]*)$/, 'speakerPrepare'],
  [/^yt-dlp gave no video information for this link\.$/, 'ytdlpNoInfo'],
  [/^Siaran ini masih live\./, 'live'],
  [/^This link has no video with a known length\.$/, 'ytdlpNoLength'],
  [/^yt-dlp tidak bisa dijalankan \((.*?)\): ([\s\S]*)\. Pasang yt-dlp atau taruh yt-dlp\.exe di folder data Cruxvid\.$/, 'ytdlpMissing'],
  [/^yt-dlp downloaded no audio for this link\.$/, 'ytdlpNoAudio'],
  [/^Potongan Clip dari link ini gagal diverifikasi/, 'cutUnverified'],
  [/^yt-dlp produced no file for this Clip\.$/, 'ytdlpNoFile'],
];

/** Text for an error from the Rust engine in the active UI language; a message nobody mapped is shown as it came. */
export function errorText(e) {
  const message = String(e?.message ?? e);
  for (const [pattern, key, table] of ERROR_RULES) {
    const m = pattern.exec(message);
    if (!m) continue;
    const args = m.slice(1);
    if (table) args[0] = strings.errors[table][args[0]];
    return strings.errors[key](...args);
  }
  return message;
}

/** "a.b" -> strings.a.b */
const lookup = (path) => path.split('.').reduce((o, k) => o?.[k], strings);

/** Puts the active language into every static element marked data-i18n / data-i18n-placeholder. */
export function applyI18n() {
  document.documentElement.lang = getLanguage();
  for (const el of document.querySelectorAll('[data-i18n]')) el.textContent = lookup(el.dataset.i18n);
  for (const el of document.querySelectorAll('[data-i18n-placeholder]')) el.placeholder = lookup(el.dataset.i18nPlaceholder);
}

/** No data-theme = the CSS follows prefers-color-scheme. */
export function applyTheme(theme) {
  if (theme) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
}

const listeners = [];
/** `cb()` runs after the user switched the UI language, once the static text is updated: redraw what is built in code. */
export const onLanguageChange = (cb) => void listeners.push(cb);

/** Loads the saved choice, applies it and wires the Settings controls. Run before any other panel reads `strings`. */
export async function initI18n() {
  const language = document.getElementById('ui-language');
  const theme = document.getElementById('ui-theme');
  const error = document.getElementById('ui-prefs-error');
  const prefs = await getUiPrefs().catch(() => ({ language: '', theme: '' }));
  setLanguage(resolveLanguage(prefs.language, navigator.languages));
  applyTheme(prefs.theme);
  applyI18n();
  language.value = getLanguage();
  theme.value = prefs.theme || 'system';

  const save = async () => {
    error.textContent = '';
    try {
      await setUiPrefs({ language: language.value, theme: theme.value === 'system' ? '' : theme.value });
    } catch (e) {
      error.textContent = errorText(e);
    }
  };
  language.addEventListener('change', async () => {
    setLanguage(language.value);
    applyI18n();
    listeners.forEach((cb) => cb());
    await save();
  });
  theme.addEventListener('change', () => {
    applyTheme(theme.value === 'system' ? '' : theme.value);
    return save();
  });
}
