// The only place the UI talks to the Rust Engine (Tauri commands).
const { invoke } = window.__TAURI__.core;

export const getConfig = () => invoke('get_config');
export const analyzeSource = (path) => invoke('analyze_source', { path });
export const exportClip = (projectId, clipId) => invoke('export_clip', { projectId, clipId });

// AI settings (cruxvid-b2t.7)
export const aiStatus = () => invoke('ai_status');
export const saveApiKey = (key) => invoke('save_api_key', { key });
export const clearApiKey = () => invoke('clear_api_key');
export const setModel = (model) => invoke('set_model', { model });
export const estimateTokens = (path) => invoke('estimate_tokens', { path });

// Layout (cruxvid-b2t.13)
export const layoutPresets = () => invoke('layout_presets');
export const getLayout = (projectId, clipId) => invoke('get_layout', { projectId, clipId });
export const setLayoutPreset = (projectId, clipId, preset) => invoke('set_layout_preset', { projectId, clipId, preset });
export const shiftLayoutPane = (projectId, clipId, pane, dx, dy) =>
  invoke('shift_layout_pane', { projectId, clipId, pane, dx, dy });
/** A JPEG from the middle of the Clip, decoded for canvas drawing. */
export const getClipFrame = async (projectId, clipId) =>
  createImageBitmap(new Blob([await invoke('get_clip_frame', { projectId, clipId })], { type: 'image/jpeg' }));

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
