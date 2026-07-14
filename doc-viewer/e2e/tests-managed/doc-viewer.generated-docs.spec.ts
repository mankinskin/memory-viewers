import { expect, test } from '@playwright/test';

import { gotoAndWaitForViewer } from '../../../../viewer-api/viewer-api/frontend/dioxus/e2e/shared/managed-viewers';
import { DOC_VIEWER } from '../../../../viewer-api/viewer-api/frontend/dioxus/e2e/shared/managed-viewers';

test.describe('doc-viewer generated docs', () => {
  test('loads generated rustdoc HTML from the managed doc-http routes', async ({ page }) => {
    await gotoAndWaitForViewer(page, DOC_VIEWER);

    await page.getByRole('button', { name: 'Reload doc-http data' }).click();

    const docTarget = page.getByText('doc_api', { exact: true }).first();
    await expect(docTarget).toBeVisible();
    await docTarget.click({ force: true });

    const frame = page.locator('iframe');
    await expect(frame).toBeVisible();
    await expect(frame).toHaveAttribute(
      'src',
      /\/api\/docs\/artifacts\/doc%2Dapi\/doc%5Fapi\/html$/,
    );

    await expect(page.frameLocator('iframe').locator('body')).toContainText('Crate doc_api');
  });
});