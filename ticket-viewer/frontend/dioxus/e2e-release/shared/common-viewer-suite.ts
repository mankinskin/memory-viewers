import { test, expect } from '@playwright/test';
import { loadAndInspectViewer } from '../../../../../viewer-api/viewer-api/frontend/dioxus/e2e/test_apis';
import type { ViewerConfig } from './viewers';

/**
 * Baseline suite contract for the managed ticket-viewer release binary.
 */
export function registerCommonViewerSuite(viewer: ViewerConfig): void {
  test.describe(`${viewer.name} — common suite`, () => {
    test('renders without console errors or uncaught exceptions', async ({ page }) => {
      test.setTimeout(90_000);

      const { errors } = await loadAndInspectViewer(
        page,
        viewer.url,
        viewer.readySelector,
        viewer.readyTimeout,
      );

      expect(errors, `${viewer.name} produced JS errors after loading`).toEqual([]);
    });

    test('no missing static assets (no 404 for JS/CSS/WASM)', async ({ page }) => {
      test.setTimeout(90_000);

      const { missingAssets } = await loadAndInspectViewer(
        page,
        viewer.url,
        viewer.readySelector,
        viewer.readyTimeout,
      );

      expect(missingAssets, `${viewer.name} has missing static assets`).toEqual([]);
    });

    test('ready-selector is visible after load', async ({ page }) => {
      test.setTimeout(90_000);

      await page.goto(viewer.url, { waitUntil: 'domcontentloaded' });
      await expect(page.locator(viewer.readySelector).first()).toBeVisible({
        timeout: viewer.readyTimeout,
      });
    });

    test('shared header actions render Home and Theme settings affordances', async ({ page }) => {
      test.setTimeout(90_000);

      await page.goto(viewer.url, { waitUntil: 'domcontentloaded' });
      await expect(page.locator(viewer.readySelector).first()).toBeVisible({
        timeout: viewer.readyTimeout,
      });

      await expect(page.getByRole('button', { name: 'Home' })).toBeVisible({
        timeout: 10_000,
      });
      await expect(page.getByRole('button', { name: 'Theme settings' })).toBeVisible();
    });
  });
}