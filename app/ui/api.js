// The only place the UI talks to the Rust Engine (Tauri commands).
const { invoke } = window.__TAURI__.core;

export const getConfig = () => invoke('get_config');
export const analyzeSource = (path) => invoke('analyze_source', { path });
export const exportClip = (projectId, clipId) => invoke('export_clip', { projectId, clipId });

/** Native file picker; resolves to a path or null when cancelled. */
export const pickVideo = () =>
  window.__TAURI__.dialog.open({
    multiple: false,
    filters: [{ name: 'Video', extensions: ['mp4', 'mkv', 'mov', 'webm', 'avi', 'ts', 'flv'] }],
  });

export const captionPresets = () => invoke('caption_presets');
export const getCaption = (projectId, clipId) => invoke('get_caption', { projectId, clipId });
export const setCaptionStyle = (projectId, clipId, style) => invoke('set_caption_style', { projectId, clipId, style });
export const correctCaptionWord = (projectId, index, text) => invoke('correct_caption_word', { projectId, index, text });

// Export queue and output folder.
export const exportClips = (projectId, clipIds) => invoke('export_clips', { projectId, clipIds });
export const retryFailedExports = (projectId) => invoke('retry_failed_exports', { projectId });
export const exportJobs = (projectId) => invoke('export_jobs', { projectId });
export const getOutputDir = () => invoke('get_output_dir');
export const setOutputDir = (dir) => invoke('set_output_dir', { dir });
export const openOutputDir = () => invoke('open_output_dir');
export const pickFolder = () => window.__TAURI__.dialog.open({ directory: true, multiple: false });
