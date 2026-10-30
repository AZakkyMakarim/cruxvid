// All user-visible text lives here so the UI-language ticket (cruxvid-b2t.18) can swap it per language.
export const strings = {
  source: 'Source',
  clips: 'Clips',
  pathPlaceholder: 'Path to a local video file',
  browse: 'Browse...',
  analyze: 'Analyze',
  analyzing: 'Analyzing...',
  noClips: 'Analysis finished but no Clip fits 60-120 s.',
  clipsFound: (n) => `${n} Clip(s) found.`,
  hook: 'Hook score',
  export: 'Export',
  exporting: 'Exporting...',
  exported: (p) => `Saved ${p}`,
};

// Caption panel.
strings.caption = 'Caption';
strings.captionPreset = 'Preset';
strings.captionFont = 'Font';
strings.captionSize = 'Size';
strings.captionColor = 'Color';
strings.captionHighlight = 'Highlight';
strings.captionOutline = 'Outline';
strings.captionPosition = 'Position';
strings.captionWords = 'Words (edit to correct; empty restores the original)';

Object.assign(strings, {
  settings: 'Settings',
  model: 'Gemini model',
  saveModel: 'Save model',
  modelSaved: 'Model saved.',
  keyTitle: 'Gemini API key needed',
  keyHelp: 'AI analysis uses your own Gemini API key. It is stored in the Windows Credential Manager.',
  keySave: 'Save key',
  keyCancel: 'Cancel',
  keyChange: 'Change key',
  keyRemove: 'Remove key',
  keySet: 'An API key is saved.',
  keyUnset: 'No API key saved. Everything except AI analysis works without one.',
  tokenEstimate: (e) =>
    `Estimated Gemini usage: about ${e.total.toLocaleString('en-US')} tokens (${e.input.toLocaleString('en-US')} in, ${e.output.toLocaleString('en-US')} out) for ${Math.round(e.duration / 60)} min of video.`,
});
