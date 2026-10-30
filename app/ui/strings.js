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

  layout: 'Layout',
  layoutPresets: { focus: 'Focus', stack: 'Split top-bottom', webcam_game: 'Split person + game', full: 'Full' },
  layoutNotices: {
    no_face: 'No face found in this Clip, using Full layout.',
    multiple_faces: 'Several faces found, using Full layout. Pick a Layout to change it.',
  },
  layoutHint: 'Drag a pane to move its crop, or focus it and use the arrow keys.',
  keySave: 'Save key',

  keyCancel: 'Cancel',

  keyChange: 'Change key',

  keyRemove: 'Remove key',

  keySet: 'An API key is saved.',

  keyUnset: 'No API key saved. Everything except AI analysis works without one.',

  tokenEstimate: (e) =>

    `Estimated Gemini usage: about ${e.total.toLocaleString('en-US')} tokens (${e.input.toLocaleString('en-US')} in, ${e.output.toLocaleString('en-US')} out) for ${Math.round(e.duration / 60)} min of video.`,

});


// Clip order and threshold.
strings.clipOrder = 'Order';
strings.clipOrders = { timeline: 'Timeline', hook: 'Hook score' };
strings.clipThreshold = 'Minimum Hook score';

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


// Source from a link (cruxvid-b2t.8).
strings.pathPlaceholder = 'Path to a local video file, or a YouTube / Twitch / video link';

// Clip title and description.
strings.clipTitle = 'Title';
strings.clipDescription = 'Description';
strings.regenerate = 'Regenerate';
strings.save = 'Save';
strings.clipTextSaved = 'Saved.';

// Speaker follow (cruxvid-b2t.14).
strings.layoutNotices.follow_speaker = 'Following whoever speaks. Pick another Layout to change it.';
strings.speakerScrub = 'Preview time (s)';
// Project history.
strings.history = 'Projects';
strings.historyOpen = 'Open';
