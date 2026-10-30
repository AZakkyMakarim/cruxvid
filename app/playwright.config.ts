import { defineConfig } from '@playwright/test';

// Tests launch the release exe themselves and attach over CDP; no browser download needed.
export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  workers: 1,
  reporter: 'list',
});
