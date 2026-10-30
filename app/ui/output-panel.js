// Settings: output folder for Exports, plus the "Open folder" button.
import { getOutputDir, openOutputDir, pickFolder, setOutputDir } from './api.js';
import { strings } from './strings.js';

export async function initOutputPanel() {
  const input = document.getElementById('output-dir');
  const msg = document.getElementById('output-msg');
  const show = async (action) => {
    try {
      msg.textContent = await action();
    } catch (e) {
      msg.textContent = String(e);
    }
  };

  document.getElementById('output-browse-btn').onclick = async () => {
    const picked = await pickFolder();
    if (picked) input.value = picked;
  };
  document.getElementById('output-save-btn').onclick = () =>
    show(async () => strings.outputSaved((input.value = await setOutputDir(input.value))));
  document.getElementById('open-folder-btn').onclick = () => show(async () => strings.folderOpened(await openOutputDir()));
  await show(async () => ((input.value = await getOutputDir()), ''));
}
