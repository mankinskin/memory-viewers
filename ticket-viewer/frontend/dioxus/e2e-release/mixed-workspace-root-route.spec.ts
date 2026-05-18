import { expect, test, type APIResponse, type Page } from '@playwright/test';
import { execFile, spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';

import { TICKET_VIEWER } from './shared/viewers';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(__dirname, '../../../../..');
const ticketCli = path.join(repoRoot, 'target', 'debug', 'ticket.exe');

interface JsonEnvelope<T> {
  payload: T;
}

interface CreatePayload {
  id: string;
}

interface TicketFileEntry {
  name: string;
  path: string;
}

interface TicketFilesResponse {
  files?: TicketFileEntry[];
}

interface TicketRefPayload {
  workspace?: string;
  id?: string;
}

interface TicketListItem {
  id: string;
  ticket_ref?: TicketRefPayload;
}

interface TicketsResponse {
  items?: TicketListItem[];
}

interface TicketHistoryResponse {
  entries?: Array<unknown>;
}

interface SeededViewerFixture {
  tempDir: string;
  url: string;
  viewer: ChildProcessWithoutNullStreams;
  childTicketId: string;
  childWorkspace: string;
  childTitle: string;
}

let fixture: SeededViewerFixture | null = null;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

async function expectJson<T>(response: APIResponse, message: string): Promise<T> {
  expect(response.ok(), message).toBe(true);
  return (await response.json()) as T;
}

async function resolveTicketViewerBin(): Promise<string> {
  const candidates = [
    process.env['TICKET_VIEWER_BIN'],
    path.join(repoRoot, 'target', 'release', 'ticket-viewer.exe'),
    process.env['USERPROFILE']
      ? path.join(process.env['USERPROFILE'], '.cargo', 'bin', 'ticket-viewer.exe')
      : undefined,
  ].filter((candidate): candidate is string => Boolean(candidate));

  for (const candidate of candidates) {
    try {
      await fs.access(candidate);
      return candidate;
    } catch {
      // Try the next candidate.
    }
  }

  throw new Error('Could not locate ticket-viewer.exe for seeded mixed-workspace validation.');
}

async function runTicketCli<T>(args: string[]): Promise<T> {
  const { stdout } = await execFileAsync(ticketCli, args, {
    cwd: repoRoot,
    windowsHide: true,
  });
  const envelope = JSON.parse(stdout) as JsonEnvelope<T>;
  return envelope.payload;
}

async function waitForViewer(url: string): Promise<void> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      const response = await fetch(`${url}/api/workspaces`);
      if (response.ok) {
        return;
      }
    } catch {
      // Server is still starting.
    }

    await sleep(500);
  }

  throw new Error(`Timed out waiting for seeded ticket-viewer at ${url}`);
}

async function startSeededViewer(indexRoot: string): Promise<{ url: string; viewer: ChildProcessWithoutNullStreams }> {
  const ticketViewerBin = await resolveTicketViewerBin();

  return await new Promise((resolve, reject) => {
    const viewer = spawn(
      ticketViewerBin,
      ['--port', '0', '--index-root', indexRoot],
      {
        cwd: repoRoot,
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
      },
    );

    let settled = false;
    let output = '';
    const timeout = setTimeout(() => {
      if (!settled) {
        viewer.kill();
        reject(new Error(`Timed out waiting for ticket-viewer port announcement. Output:\n${output}`));
      }
    }, 60_000);

    const onData = async (chunk: Buffer) => {
      output += chunk.toString();
      const match = output.match(/TICKET_VIEWER_PORT=(\d+)/);
      if (!match || settled) {
        return;
      }

      settled = true;
      clearTimeout(timeout);
      const url = `http://127.0.0.1:${match[1]}`;
      try {
        await waitForViewer(url);
        resolve({ url, viewer });
      } catch (error) {
        viewer.kill();
        reject(error);
      }
    };

    viewer.stdout.on('data', (chunk) => {
      void onData(chunk);
    });
    viewer.stderr.on('data', (chunk) => {
      output += chunk.toString();
    });
    viewer.on('exit', (code) => {
      if (!settled) {
        clearTimeout(timeout);
        reject(new Error(`ticket-viewer exited before startup completed (code ${code}). Output:\n${output}`));
      }
    });
  });
}

async function stopSeededViewer(viewer: ChildProcessWithoutNullStreams): Promise<void> {
  if (viewer.exitCode !== null) {
    return;
  }

  viewer.kill();
  await new Promise<void>((resolve) => {
    viewer.once('exit', () => resolve());
    setTimeout(resolve, 5_000);
  });
}

async function seedMixedWorkspaceFixture(): Promise<SeededViewerFixture> {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'ticket-viewer-mixed-'));
  const parentDir = path.join(tempDir, 'parent');
  const childDir = path.join(parentDir, 'child');
  const parentTicketsDir = path.join(parentDir, 'tickets');
  const childTicketsDir = path.join(childDir, 'tickets');
  const parentBodyPath = path.join(tempDir, 'parent-description.md');
  const childBodyPath = path.join(tempDir, 'child-description.md');
  const childAssetPath = path.join(tempDir, 'child-asset.md');
  const childWorkspace = 'child';
  const childTitle = 'Seeded child ticket';

  await fs.mkdir(parentTicketsDir, { recursive: true });
  await fs.mkdir(childTicketsDir, { recursive: true });
  await fs.writeFile(parentBodyPath, '# Parent ticket\n\nDefault workspace root for mixed-workspace validation.\n');
  await fs.writeFile(childBodyPath, '# Child ticket\n\nMixed-workspace ticket body used by Playwright validation.\n');
  await fs.writeFile(childAssetPath, '# Asset\n\nMixed-workspace asset payload.\n');

  await runTicketCli<CreatePayload>([
    'create',
    '--json',
    '--index-root',
    parentDir,
    '--type',
    'tracker-improvement',
    '--title',
    'Seeded parent ticket',
    '--body-file',
    parentBodyPath,
  ]);

  const childTicket = await runTicketCli<CreatePayload>([
    'create',
    '--json',
    '--index-root',
    childDir,
    '--type',
    'tracker-improvement',
    '--title',
    childTitle,
    '--body-file',
    childBodyPath,
  ]);

  await runTicketCli<unknown>([
    'attach',
    '--json',
    '--index-root',
    childDir,
    childTicket.id,
    childAssetPath,
    '--as',
    'notes.md',
  ]);

  await runTicketCli<unknown>([
    'add-root',
    '--json',
    '--index-root',
    parentDir,
    '--label',
    childWorkspace,
    childTicketsDir,
  ]);

  await runTicketCli<unknown>([
    'scan',
    '--json',
    '--index-root',
    parentDir,
  ]);

  const { url, viewer } = await startSeededViewer(parentDir);

  return {
    tempDir,
    url,
    viewer,
    childTicketId: childTicket.id,
    childWorkspace,
    childTitle,
  };
}

async function gotoAndWaitForSeededViewer(page: Page, url: string): Promise<void> {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.locator(TICKET_VIEWER.readySelector).first().waitFor({
    state: 'visible',
    timeout: TICKET_VIEWER.readyTimeout,
  });
}

test.beforeAll(async () => {
  fixture = await seedMixedWorkspaceFixture();
});

test.afterAll(async () => {
  if (!fixture) {
    return;
  }

  await stopSeededViewer(fixture.viewer);
  await fs.rm(fixture.tempDir, { recursive: true, force: true });
  fixture = null;
});

test('root route follows mixed-workspace ticket refs for history and file actions', async ({ page }) => {
  test.setTimeout(120_000);

  expect(fixture, 'seeded viewer fixture must be ready').not.toBeNull();
  const currentFixture = fixture!;
  const escapedBaseUrl = escapeRegex(currentFixture.url);

  await gotoAndWaitForSeededViewer(page, currentFixture.url);
  await expect(page).toHaveURL(new RegExp(`^${escapedBaseUrl}/(?:#.*)?$`));

  const ticketsResponse = await expectJson<TicketsResponse>(
    await page.request.get(
      `${currentFixture.url}/api/tickets?workspace=default&limit=200`,
    ),
    'seeded ticket list must respond',
  );
  const listedChildTicket = (ticketsResponse.items ?? []).find(
    (ticket) => ticket.id === currentFixture.childTicketId,
  );
  expect(listedChildTicket, 'seeded child ticket must appear in the default workspace list').toBeTruthy();
  expect(
    listedChildTicket?.ticket_ref?.workspace,
    'seeded child ticket list item must retain its owning workspace ref',
  ).toBe(currentFixture.childWorkspace);

  const ticketButton = page.getByTestId(`ticket-tree-ticket-${currentFixture.childTicketId}`);
  await expect(ticketButton).toBeVisible({ timeout: 30_000 });
  await expect(ticketButton).toContainText(currentFixture.childTitle);

  const historyResponsePromise = page.waitForResponse((response) => {
    return (
      response.request().method() === 'GET' &&
      response.url().includes(`/api/tickets/${currentFixture.childTicketId}/history?`) &&
      response.url().includes(`workspace=${encodeURIComponent(currentFixture.childWorkspace)}`)
    );
  });
  await ticketButton.click();
  const historyResponse = await historyResponsePromise;
  expect(historyResponse.ok(), 'mixed-workspace history request must succeed').toBe(true);
  await expect(page).toHaveURL(
    new RegExp(
      `^${escapedBaseUrl}/#(?=.*ticket-id=${currentFixture.childTicketId})(?=.*ticket-workspace=${currentFixture.childWorkspace}).*$`,
    ),
  );
  await expect(page.getByTestId('ticket-detail-panel')).toBeVisible();
  await expect(page.getByTestId('ticket-content')).toBeVisible();

  const seededHistory = await expectJson<TicketHistoryResponse>(
    await page.request.get(
      `${currentFixture.url}/api/tickets/${currentFixture.childTicketId}/history?workspace=${encodeURIComponent(currentFixture.childWorkspace)}`,
    ),
    'seeded child history must respond',
  );
  expect(
    seededHistory.entries?.length ?? 0,
    'seeded child ticket should have visible history entries for the history tab flow',
  ).toBeGreaterThan(0);

  const row = page.getByTestId(`ticket-tree-row-${currentFixture.childTicketId}`);
  const filesResponsePromise = page.waitForResponse((response) => {
    return (
      response.request().method() === 'GET' &&
      response.url().includes(`/api/tickets/${currentFixture.childTicketId}/files?`) &&
      response.url().includes(`workspace=${encodeURIComponent(currentFixture.childWorkspace)}`)
    );
  });
  await row.locator('button').first().click();
  const filesResponse = await filesResponsePromise;
  expect(filesResponse.ok(), 'mixed-workspace file list request must succeed').toBe(true);

  await expectJson<TicketFilesResponse>(filesResponse, 'mixed-workspace file list JSON must parse');
  const historyTab = page.getByTestId('tab-history');
  await expect(historyTab).toBeVisible();
  await historyTab.click();
  await expect(page.getByTestId('history-panel')).toBeVisible({ timeout: 20_000 });
  await expect(page).toHaveURL(
    new RegExp(
      `^${escapedBaseUrl}/#(?=.*ticket-id=${currentFixture.childTicketId})(?=.*ticket-workspace=${currentFixture.childWorkspace}).*$`,
    ),
  );
  await expect(page).not.toHaveURL(/\/workspace\/default/);
});