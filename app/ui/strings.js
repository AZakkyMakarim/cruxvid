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

// Export queue and output folder.
strings.queue = 'Export queue';
strings.queueExportSelected = 'Export selected';
strings.queueRetryFailed = 'Retry failed';
strings.queueStatus = { waiting: 'Waiting', running: 'Running', done: 'Done', failed: 'Failed' };
strings.outputFolder = 'Output folder';
strings.outputBrowse = 'Browse...';
strings.outputSave = 'Save';
strings.openFolder = 'Open folder';
strings.outputSaved = (p) => `Output folder: ${p}`;
strings.folderOpened = (p) => `Opened ${p}`;
