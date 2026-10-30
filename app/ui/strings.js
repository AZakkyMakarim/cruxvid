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
  layout: 'Layout',
  layoutPresets: { focus: 'Focus', stack: 'Split top-bottom', webcam_game: 'Split person + game', full: 'Full' },
  layoutNotices: {
    no_face: 'No face found in this Clip, using Full layout.',
    multiple_faces: 'Several faces found, using Full layout. Pick a Layout to change it.',
  },
  layoutHint: 'Drag a pane to move its crop, or focus it and use the arrow keys.',
};
