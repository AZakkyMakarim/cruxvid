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

/** Native file picker; resolves to a path or null when cancelled. */
export const pickVideo = () =>
  window.__TAURI__.dialog.open({
    multiple: false,
    filters: [{ name: 'Video', extensions: ['mp4', 'mkv', 'mov', 'webm', 'avi', 'ts', 'flv'] }],
  });
