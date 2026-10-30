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

// Clip order and threshold (cruxvid-b2t.9)
export const arrangeClips = (projectId, order, threshold) => invoke('arrange_clips', { projectId, order, threshold });

// Export queue and output folder.
export const exportClips = (projectId, clipIds) => invoke('export_clips', { projectId, clipIds });
export const retryFailedExports = (projectId) => invoke('retry_failed_exports', { projectId });
export const exportJobs = (projectId) => invoke('export_jobs', { projectId });
export const getOutputDir = () => invoke('get_output_dir');
export const setOutputDir = (dir) => invoke('set_output_dir', { dir });
export const openOutputDir = () => invoke('open_output_dir');
export const pickFolder = () => window.__TAURI__.dialog.open({ directory: true, multiple: false });

// Clip title/description (cruxvid-b2t.10)
export const editClipText = (projectId, clipId, title, description) => invoke('edit_clip_text', { projectId, clipId, title, description });
export const regenerateClipText = (projectId, clipId, field) => invoke('regenerate_clip_text', { projectId, clipId, field });

// Caption translation per Clip (cruxvid-b2t.12); language "" = original.
export const getClipLanguage = (projectId, clipId) => invoke('get_clip_language', { projectId, clipId });
export const setClipLanguage = (projectId, clipId, language) => invoke('set_clip_language', { projectId, clipId, language });

// Speaker follow (cruxvid-b2t.14).
export const getLayoutAt = (projectId, clipId, t) => invoke('get_layout_at', { projectId, clipId, t });
export const getClipFrameAt = async (projectId, clipId, t) =>
  createImageBitmap(new Blob([await invoke('get_clip_frame_at', { projectId, clipId, t })], { type: 'image/jpeg' }));
// Project history (cruxvid-b2t.16)
export const listProjects = () => invoke('list_projects');
export const openProject = (id) => invoke('open_project', { id });

// Preview, trim and manual Clips (cruxvid-b2t.17)
export const createManualProject = (path) => invoke('create_manual_project', { path });
export const addManualClip = (projectId, start, end) => invoke('add_manual_clip', { projectId, start, end });
export const setClipRange = (projectId, clipId, start, end) => invoke('set_clip_range', { projectId, clipId, start, end });
export const rangeWarning = (start, end) => invoke('range_warning', { start, end });
export const clipPreview = (projectId, clipId) => invoke('clip_preview', { projectId, clipId });
export const previewUrl = (path) => window.__TAURI__.core.convertFileSrc(path);
