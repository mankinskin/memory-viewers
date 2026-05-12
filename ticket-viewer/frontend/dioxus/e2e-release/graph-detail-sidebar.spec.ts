import { expect, test, type Page } from '@playwright/test';
import { TICKET_VIEWER } from './shared/viewers';

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
  rootId: string;
  childId: string;
  childTitle: string;
  childComponent: string;
}

async function getTicketComponent(page: Page, id: string): Promise<string> {
  const resp = await page.request.get(
    `${TICKET_VIEWER.url}/api/tickets/${id}?workspace=default`,
  );
  if (!resp.ok()) {
    return '';
  }
  const body = await resp.json();
  return body?.ticket?.fields?.component ?? '';
}

async function findGraphSelectionCandidate(page: Page): Promise<CandidateSelection> {
  const listResp = await page.request.get(
    `${TICKET_VIEWER.url}/api/tickets?workspace=default&limit=500`,
  );
  expect(listResp.ok(), 'ticket list API must respond').toBe(true);

  const listBody = await listResp.json();
  const items: TicketListItem[] = listBody?.items ?? [];
  expect(items.length, 'workspace `default` must contain tickets').toBeGreaterThan(0);

  for (const item of items) {
    const rootComponent = await getTicketComponent(page, item.id);

    const graphResp = await page.request.get(
      `${TICKET_VIEWER.url}/api/graph/subgraph?workspace=default&root=${item.id}&depth=1`,
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
      const childComponent = await getTicketComponent(page, childId);
      if (!childComponent || childComponent === rootComponent) {
        continue;
      }

      const childNode = nodes.find((node) => node.id === childId);
      const childTitle = childNode?.title?.trim() || childId;
      return {
        rootId: item.id,
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

    await page.goto(`${TICKET_VIEWER.url}/workspace/default`, {
      waitUntil: 'domcontentloaded',
    });
    await page.evaluate(
      ({ key, id }) => {
        const raw = localStorage.getItem(key);
        const obj = raw ? JSON.parse(raw) : {};
        obj.open_ticket_id = id;
        localStorage.setItem(key, JSON.stringify(obj));
      },
      { key: 'ticket-viewer:default:ui', id: candidate.rootId },
    );
    await page.goto(`${TICKET_VIEWER.url}/workspace/default#id=${candidate.rootId}`, {
      waitUntil: 'domcontentloaded',
    });
    await page.locator(TICKET_VIEWER.readySelector).first().waitFor({
      state: 'visible',
      timeout: TICKET_VIEWER.readyTimeout,
    });

    const detailPanel = page.getByTestId('ticket-detail-panel');
    const componentField = page.getByTestId('ticket-detail-field-component');
    await expect(detailPanel).toBeVisible();
    await expect(componentField).toBeVisible();
    await expect(componentField).not.toContainText(candidate.childComponent);

    await expect(page.locator('#graph3d-container')).toBeAttached({ timeout: 30_000 });
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

    await expect(componentField).toContainText(candidate.childComponent, { timeout: 20_000 });
    await expect(page).toHaveURL(new RegExp(`#id=${candidate.rootId}$`));
  });
});