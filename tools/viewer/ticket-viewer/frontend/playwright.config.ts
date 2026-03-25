import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for ticket-viewer end-to-end tests.
 *
 * Tests run against a Vite dev server started automatically before the suite.
 * API calls are intercepted via `page.route()` inside each test — no real
 * `ticket serve` backend is required.
 *
 * Run with:
 *   npm run test:e2e            (headless)
 *   npm run test:e2e:headed     (visible browser)
 *   npm run test:e2e:ui         (Playwright UI mode)
 */

const DEV_SERVER_URL = 'http://localhost:5173';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env['CI'],
  retries: process.env['CI'] ? 2 : 0,
  workers: process.env['CI'] ? 1 : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],

  use: {
    baseURL: DEV_SERVER_URL,
    // Collect traces on first retry so failures are diagnosable.
    trace: 'on-first-retry',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  // Start the Vite dev server before running the suite.
  webServer: {
    command: 'npm run dev',
    url: DEV_SERVER_URL,
    reuseExistingServer: !process.env['CI'],
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
