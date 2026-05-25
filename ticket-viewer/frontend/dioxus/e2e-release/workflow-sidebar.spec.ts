import { expect, test, type APIResponse, type Page, type TestInfo } from '@playwright/test';
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

interface CreateTicketResponse {
  ticket?: { id?: string };
}

interface WorkspacesResponse {
  active_workspace?: string;
  workspaces?: Array<{ name: string }>;
}

interface WorkflowCandidateItem {
  id: string;
}

interface WorkflowTreeItem {
  id: string;
  children?: WorkflowTreeItem[];
}

interface WorkflowNextResponse {
  count: number;
  items: WorkflowCandidateItem[];
}

interface WorkflowTreeResponse {
  root: WorkflowTreeItem;
  frontier_items?: WorkflowCandidateItem[];
}

interface WorkflowFixture {
  tempDir: string;
  indexRoot: string;
  url: string;
  viewer: ChildProcessWithoutNullStreams;
  workspace: string;
  blockersRootId: string;
  blockersRootTitle: string;
  unblockedByRootId: string;
  unblockedByRootTitle: string;
}

async function expectJson<T>(response: APIResponse, message: string): Promise<T> {
  expect(response.ok(), message).toBe(true);
  return (await response.json()) as T;
}

async function runTicketCli<T>(args: string[]): Promise<T> {
  const { stdout } = await execFileAsync(ticketCli, args, {
    cwd: repoRoot,
    windowsHide: true,
  });
  const envelope = JSON.parse(stdout) as JsonEnvelope<T>;
  return envelope.payload;
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

  throw new Error('Could not locate ticket-viewer.exe for workflow sidebar validation.');
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

    await new Promise((resolve) => setTimeout(resolve, 500));
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

async function resolveActiveWorkspace(url: string): Promise<string> {
  const response = await fetch(`${url}/api/workspaces`);
  expect(response.ok, 'workspace list request failed').toBe(true);
  const body = (await response.json()) as WorkspacesResponse;
  const workspace = body.active_workspace || body.workspaces?.[0]?.name;
  expect(workspace, 'expected an active workspace name').toBeTruthy();
  return workspace!;
}

async function createTicket(
  page: Page,
  url: string,
  workspace: string,
  title: string,
): Promise<string> {
  const response = await page.request.post(
    `${url}/api/tickets?workspace=${encodeURIComponent(workspace)}`,
    {
      data: {
        type: 'tracker-improvement',
        title,
      },
    },
  );
  const body = await expectJson<CreateTicketResponse>(response, `create ticket failed for ${title}`);
  const ticketId = body.ticket?.id;
  expect(ticketId, `expected created ticket id for ${title}`).toBeTruthy();
  return ticketId!;
}

async function patchTicketState(
  page: Page,
  url: string,
  workspace: string,
  ticketId: string,
  state: string,
): Promise<void> {
  const response = await page.request.patch(
    `${url}/api/tickets/${ticketId}?workspace=${encodeURIComponent(workspace)}`,
    {
      data: { state },
    },
  );
  expect(response.ok(), `patch ticket state failed for ${ticketId} -> ${state}`).toBe(true);
}

async function createDependsOnEdge(
  page: Page,
  url: string,
  workspace: string,
  fromId: string,
  toId: string,
): Promise<void> {
  const response = await page.request.post(
    `${url}/api/edges?workspace=${encodeURIComponent(workspace)}`,
    {
      data: {
        from_id: fromId,
        to_id: toId,
        kind: 'depends_on',
      },
    },
  );
  expect(response.ok(), `create edge failed for ${fromId} -> ${toId}`).toBe(true);
}

async function seedWorkflowFixture(page: Page): Promise<WorkflowFixture> {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'ticket-viewer-workflow-'));
  const indexRoot = path.join(tempDir, 'workspace');
  const blockerRootTitle = 'Workflow blocker root';
  const siblingBlockerTitle = 'Workflow sibling blocker';
  const blockersRootTitle = 'Workflow blocked target';
  const downstreamMidTitle = 'Workflow downstream mid';
  const downstreamLeafTitle = 'Workflow downstream leaf';
  await fs.mkdir(indexRoot, { recursive: true });
  await runTicketCli<unknown>([
    'init',
    '--json',
    '--index-root',
    indexRoot,
  ]);

  const { url, viewer } = await startSeededViewer(indexRoot);
  const workspace = await resolveActiveWorkspace(url);

  const blockerRootId = await createTicket(page, url, workspace, blockerRootTitle);
  const siblingBlockerId = await createTicket(page, url, workspace, siblingBlockerTitle);
  const blockersRootId = await createTicket(page, url, workspace, blockersRootTitle);
  const downstreamMidId = await createTicket(page, url, workspace, downstreamMidTitle);
  const downstreamLeafId = await createTicket(page, url, workspace, downstreamLeafTitle);

  await patchTicketState(page, url, workspace, blockerRootId, 'ready');
  await patchTicketState(page, url, workspace, siblingBlockerId, 'ready');

  await createDependsOnEdge(page, url, workspace, blockersRootId, blockerRootId);
  await createDependsOnEdge(page, url, workspace, blockersRootId, siblingBlockerId);
  await createDependsOnEdge(page, url, workspace, downstreamMidId, blockerRootId);
  await createDependsOnEdge(page, url, workspace, downstreamLeafId, downstreamMidId);

  return {
    tempDir,
    indexRoot,
    url,
    viewer,
    workspace,
    blockersRootId,
    blockersRootTitle,
    unblockedByRootId: blockerRootId,
    unblockedByRootTitle: blockerRootTitle,
  };
}

async function fetchWorkflowNext(
  page: Page,
  fixture: WorkflowFixture,
): Promise<WorkflowNextResponse> {
  const params = new URLSearchParams({
    workspace: fixture.workspace,
    limit: '50',
  });
  const response = await page.request.get(
    `${fixture.url}/api/workflow/next?${params.toString()}`,
  );
  return await expectJson<WorkflowNextResponse>(response, 'workflow next request failed');
}

async function fetchWorkflowTree(
  page: Page,
  fixture: WorkflowFixture,
  kind: 'blockers' | 'unblocked-by',
  root: string,
): Promise<WorkflowTreeResponse> {
  const params = new URLSearchParams({
    workspace: fixture.workspace,
    root,
  });
  const response = await page.request.get(
    `${fixture.url}/api/workflow/${kind}?${params.toString()}`,
  );
  return await expectJson<WorkflowTreeResponse>(response, `workflow ${kind} request failed`);
}

async function visibleIds(
  page: Page,
  prefix: string,
): Promise<string[]> {
  return await page.locator(`[data-testid^="${prefix}"]`).evaluateAll((nodes, rawPrefix) =>
    nodes
      .map((node) => node.getAttribute('data-testid') ?? '')
      .map((testId) => testId.replace(rawPrefix as string, '')),
    prefix,
  );
}

async function treeDepthIds(
  page: Page,
  depth: number,
): Promise<string[]> {
  return await page
    .locator(`[data-testid^="workflow-tree-node-"][data-depth="${depth}"]`)
    .evaluateAll((nodes) =>
      nodes
        .map((node) => node.getAttribute('data-testid') ?? '')
        .map((testId) => testId.replace('workflow-tree-node-', '')),
    );
}

async function selectTicketInBrowse(
  page: Page,
  query: string,
  ticketId: string,
): Promise<void> {
  await page.getByTestId('sidebar-mode-browse').click();
  const filterInput = page.getByTestId('ticket-tree-filter');
  await expect(filterInput).toBeVisible();
  await filterInput.fill(query);
  const ticketButton = page.getByTestId(`ticket-tree-ticket-${ticketId}`);
  await expect(ticketButton).toBeVisible();
  await ticketButton.click();
}

async function attachScreenshot(
  page: Page,
  testInfo: TestInfo,
  name: string,
): Promise<void> {
  await testInfo.attach(name, {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  });
}

test.describe('ticket-viewer — workflow sidebar', () => {
  test('workflow modes preserve server order and hierarchy', async ({ page }, testInfo) => {
    test.setTimeout(180_000);

    const fixture = await seedWorkflowFixture(page);
    try {
      const nextResponse = await fetchWorkflowNext(page, fixture);
      const blockersResponse = await fetchWorkflowTree(
        page,
        fixture,
        'blockers',
        fixture.blockersRootId,
      );
      const unblockedByResponse = await fetchWorkflowTree(
        page,
        fixture,
        'unblocked-by',
        fixture.unblockedByRootId,
      );

      expect(nextResponse.items.length).toBeGreaterThan(0);
      expect(blockersResponse.root.children?.length ?? 0).toBeGreaterThan(0);
      expect(unblockedByResponse.root.children?.length ?? 0).toBeGreaterThan(0);

      await page.goto(`${fixture.url}/`, {
        waitUntil: 'domcontentloaded',
      });
      await page.locator(TICKET_VIEWER.readySelector).first().waitFor({
        state: 'visible',
        timeout: TICKET_VIEWER.readyTimeout,
      });

      await page.getByTestId('sidebar-mode-next').click();
      await expect(
        page.getByTestId(`workflow-next-item-${nextResponse.items[0].id}`),
      ).toBeVisible();
      await expect
        .poll(async () => await visibleIds(page, 'workflow-next-item-'))
        .toEqual(nextResponse.items.map((item) => item.id));
      await attachScreenshot(page, testInfo, 'workflow-next-sidebar');

      await selectTicketInBrowse(page, fixture.blockersRootTitle, fixture.blockersRootId);
      await page.getByTestId('sidebar-mode-blockers').click();
      await expect(
        page.getByTestId(`workflow-tree-node-${fixture.blockersRootId}`),
      ).toBeVisible();
      await expect.poll(async () => await treeDepthIds(page, 1)).toEqual(
        (blockersResponse.root.children ?? []).map((item) => item.id),
      );
      await expect
        .poll(async () => await visibleIds(page, 'workflow-frontier-item-'))
        .toEqual((blockersResponse.frontier_items ?? []).map((item) => item.id));
      await attachScreenshot(page, testInfo, 'workflow-blockers-sidebar');

      await selectTicketInBrowse(
        page,
        fixture.unblockedByRootTitle,
        fixture.unblockedByRootId,
      );
      await page.getByTestId('sidebar-mode-unblocked-by').click();
      await expect(
        page.getByTestId(`workflow-tree-node-${fixture.unblockedByRootId}`),
      ).toBeVisible();
      await expect.poll(async () => await treeDepthIds(page, 1)).toEqual(
        (unblockedByResponse.root.children ?? []).map((item) => item.id),
      );
      await expect
        .poll(async () => await visibleIds(page, 'workflow-frontier-item-'))
        .toEqual((unblockedByResponse.frontier_items ?? []).map((item) => item.id));
      await attachScreenshot(page, testInfo, 'workflow-unblocked-by-sidebar');
    } finally {
      await stopSeededViewer(fixture.viewer);
      await fs.rm(fixture.tempDir, { recursive: true, force: true });
    }
  });
});