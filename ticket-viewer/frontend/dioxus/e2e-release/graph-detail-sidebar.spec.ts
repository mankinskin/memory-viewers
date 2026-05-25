import { expect, test, type Page, type TestInfo } from '@playwright/test';
import {
  gotoAndWaitForViewer,
  TICKET_VIEWER,
} from './shared/viewers';

test.use({
  headless: false,
  launchOptions: {
    args: [
      '--enable-unsafe-webgpu',
      '--enable-features=Vulkan',
      '--use-vulkan=swiftshader',
      '--use-webgpu-adapter=swiftshader',
    ],
  },
});

interface TicketListItem {
  id: string;
  title?: string;
}

interface GraphNode {
  id: string;
  title?: string;
}

interface GraphEdge {
  source?: string;
  target?: string;
  from?: string;
  to?: string;
}

interface CandidateSelection {
  workspace: string;
  rootId: string;
  rootTitle: string;
  childId: string;
  childTitle: string;
  childComponent: string;
}

interface WorkspacesResponse {
  active_workspace?: string;
  workspaces?: Array<{ name?: string }>;
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

async function openCandidateTicket(
  page: Page,
  candidate: CandidateSelection,
): Promise<void> {
  await page.goto(`${TICKET_VIEWER.url}/workspace/${candidate.workspace}`, {
    waitUntil: 'domcontentloaded',
  });
  await page.locator(TICKET_VIEWER.readySelector).first().waitFor({
    state: 'visible',
    timeout: TICKET_VIEWER.readyTimeout,
  });

  const searchBox = page.getByRole('textbox').first();
  await searchBox.fill(`id:${candidate.rootId}`);

  const rootTicketButton = page.getByTestId(`ticket-tree-ticket-${candidate.rootId}`);
  await expect(rootTicketButton).toBeVisible({ timeout: 30_000 });
  await rootTicketButton.click();
}

async function resolveActiveWorkspace(page: Page): Promise<string> {
  await gotoAndWaitForViewer(page, TICKET_VIEWER);

  const resp = await page.request.get(`${TICKET_VIEWER.url}/api/workspaces`);
  expect(resp.ok(), 'workspace list API must respond').toBe(true);

  const body = (await resp.json()) as WorkspacesResponse;
  const workspace = body.active_workspace ?? body.workspaces?.[0]?.name;
  expect(workspace, 'viewer must expose at least one workspace').toBeTruthy();
  return workspace!;
}

async function getTicketComponent(
  page: Page,
  workspace: string,
  id: string,
): Promise<string> {
  const resp = await page.request.get(
    `${TICKET_VIEWER.url}/api/tickets/${id}?workspace=${workspace}`,
  );
  if (!resp.ok()) {
    return '';
  }
  const body = await resp.json();
  return body?.ticket?.fields?.component ?? '';
}

async function findGraphSelectionCandidate(page: Page): Promise<CandidateSelection> {
  const workspace = await resolveActiveWorkspace(page);

  const listResp = await page.request.get(
    `${TICKET_VIEWER.url}/api/tickets?workspace=${workspace}&limit=500`,
  );
  expect(listResp.ok(), 'ticket list API must respond').toBe(true);

  const listBody = await listResp.json();
  const items: TicketListItem[] = listBody?.items ?? [];
  expect(items.length, `workspace \`${workspace}\` must contain tickets`).toBeGreaterThan(0);

  for (const item of items) {
    const rootComponent = await getTicketComponent(page, workspace, item.id);

    const graphResp = await page.request.get(
      `${TICKET_VIEWER.url}/api/graph/subgraph?workspace=${workspace}&root=${item.id}&depth=1`,
    );
    if (!graphResp.ok()) {
      continue;
    }

    const graph = await graphResp.json();
    const nodes: GraphNode[] = graph?.nodes ?? [];
    const edges: GraphEdge[] = graph?.edges ?? [];
    if (edges.length < 1) {
      continue;
    }

    const directChildIds = new Set(
      edges
        .filter((edge) => (edge.from ?? edge.source) === item.id)
        .map((edge) => edge.to ?? edge.target)
        .filter((id): id is string => Boolean(id) && id !== item.id),
    );

    for (const childId of directChildIds) {
      const childComponent = await getTicketComponent(page, workspace, childId);
      if (!childComponent || childComponent === rootComponent) {
        continue;
      }

      const childNode = nodes.find((node) => node.id === childId);
      const childTitle = childNode?.title?.trim() || childId;
      return {
        workspace,
        rootId: item.id,
        rootTitle: item.title?.trim() || item.id,
        childId,
        childTitle,
        childComponent,
      };
    }
  }

  throw new Error('No root ticket with a direct graph child and unique component field was found');
}

test.describe('ticket-viewer — graph selection updates right detail sidebar', () => {
  test('clicking a graph node updates the detail pane without changing the main ticket route', async ({ page }) => {
    test.setTimeout(120_000);

    const candidate = await findGraphSelectionCandidate(page);

    await openCandidateTicket(page, candidate);

    const detailPanel = page.getByTestId('ticket-detail-panel');
    const componentField = page.getByTestId('ticket-detail-field-component');
    await expect(detailPanel).toBeVisible();
    await expect(componentField).toBeVisible();
    await expect(componentField).not.toContainText(candidate.childComponent);

    await page.getByRole('button', { name: /^Graph$/ }).first().click();
    await expect(page.locator('#graph3d-container')).toBeVisible({ timeout: 30_000 });
    await page.waitForFunction(
      (childTitle) => {
        const root = document.getElementById('graph3d-container');
        if (!root) {
          return false;
        }
        return Array.from(root.querySelectorAll('[data-node-idx]')).some((node) => {
          const element = node as HTMLElement;
          return element.style.display !== 'none' && (element.textContent || '').includes(childTitle);
        });
      },
      candidate.childTitle,
      { timeout: 30_000 },
    );

    const childNode = page
      .locator('#graph3d-container [data-node-idx]:visible')
      .filter({ hasText: candidate.childTitle })
      .first();
    await expect(childNode).toBeVisible();
    await childNode.click();

    await page.getByRole('button', { name: /^Split$/ }).first().click();
    await expect(detailPanel).toBeVisible();
    await expect(componentField).toBeVisible();
    await expect(componentField).toContainText(candidate.childComponent, { timeout: 20_000 });
    await expect
      .poll(() => {
        const currentUrl = new URL(page.url());
        const hashParams = new URLSearchParams(currentUrl.hash.slice(1));
        return {
          pathname: currentUrl.pathname,
          ticketId: hashParams.get('ticket-id') ?? hashParams.get('id'),
        };
      })
      .toEqual({
        pathname: `/workspace/${candidate.workspace}`,
        ticketId: candidate.rootId,
      });
  });

  test('graph mode defaults to hierarchical isometric controls and preserves top-to-bottom depth ordering', async ({ page }, testInfo) => {
    test.setTimeout(120_000);

    const candidate = await findGraphSelectionCandidate(page);

    await openCandidateTicket(page, candidate);

    await page.getByRole('button', { name: /^Graph$/ }).first().click();
    await expect(page.locator('#graph3d-container')).toBeVisible({ timeout: 30_000 });

    await page.waitForFunction(
      ({ rootTitle, childTitle }) => {
        const root = document.getElementById('graph3d-container');
        if (!root) {
          return false;
        }
        const text = Array.from(root.querySelectorAll('[data-node-idx]'))
          .filter((node) => (node as HTMLElement).style.display !== 'none')
          .map((node) => node.textContent || '');
        return text.some((value) => value.includes(rootTitle)) &&
          text.some((value) => value.includes(childTitle));
      },
      { rootTitle: candidate.rootTitle, childTitle: candidate.childTitle },
      { timeout: 30_000 },
    );

    const rootNode = page
      .locator('#graph3d-container [data-node-idx]:visible')
      .filter({ hasText: candidate.rootTitle })
      .first();
    const childNode = page
      .locator('#graph3d-container [data-node-idx]:visible')
      .filter({ hasText: candidate.childTitle })
      .first();

    await expect(rootNode).toBeVisible();
    await expect(childNode).toBeVisible();

    const rootBox = await rootNode.boundingBox();
    const childBox = await childNode.boundingBox();
    expect(rootBox, 'root graph node must expose a screen box').not.toBeNull();
    expect(childBox, 'child graph node must expose a screen box').not.toBeNull();
    expect(
      childBox!.y,
      'child node should render below the root in the default hierarchy view',
    ).toBeGreaterThan(rootBox!.y + 10);

    await page.getByTitle('Graph settings').click();
    const hierarchicalButton = page.getByRole('button', { name: 'Hierarchical 3D' });
    const orthographicButton = page.getByRole('button', { name: 'Orthographic' });
    await expect(hierarchicalButton).toBeVisible();
    await expect(orthographicButton).toBeVisible();
    await expect(hierarchicalButton).toHaveAttribute('style', /#93bbff|79,140,255/);
    await expect(orthographicButton).toHaveAttribute('style', /#93bbff|79,140,255/);

    await attachScreenshot(page, testInfo, 'graph-default-isometric-layout');
  });
});