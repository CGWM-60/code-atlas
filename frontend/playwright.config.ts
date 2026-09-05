import { defineConfig } from '@playwright/test';
export default defineConfig({
 testDir: './e2e', workers: 1, fullyParallel: false, timeout: 60000,
 use: { baseURL: 'http://127.0.0.1:3107', viewport: { width: 1600, height: 1000 }, trace: 'retain-on-failure', screenshot: 'only-on-failure' },
 outputDir: '../output/playwright/results',
 webServer: { command: 'cargo run -- serve --bind 127.0.0.1:3107 --db /tmp/code-atlas-playwright.sqlite', cwd: '..', url: 'http://127.0.0.1:3107/api/health', reuseExistingServer: false, timeout: 120000 },
});
