import type { Page } from '@playwright/test';

export interface ViewerConfig {
  name: string;
  url: string;
  readySelector: string;
  readyTimeout: number;
}

export const TICKET_VIEWER: ViewerConfig = {
  name: 'ticket-viewer',
  url: 'http://127.0.0.1:3002',
  // viewer-api Header component renders <header class="header">.
  readySelector: 'header.header',
  readyTimeout: 60_000,
};

export async function gotoAndWaitForViewer(
  page: Page,
  viewer: ViewerConfig = TICKET_VIEWER,
): Promise<void> {
  await page.goto(viewer.url, { waitUntil: 'domcontentloaded' });
  await page.locator(viewer.readySelector).first().waitFor({
    state: 'visible',
    timeout: viewer.readyTimeout,
  });
}