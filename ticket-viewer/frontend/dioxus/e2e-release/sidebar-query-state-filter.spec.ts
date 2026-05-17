import { expect, test, type Page } from '@playwright/test';
import { TICKET_VIEWER } from './shared/viewers';

const TICKET_VIEWER_URL = process.env.TICKET_VIEWER_URL ?? TICKET_VIEWER.url;

interface TicketListItem {
  id: string;
  title?: string;
  state?: string;
}

interface QueryStateCandidate {
  query: string;
  state: string;
}

const STATE_CHIP_BY_VALUE: Record<string, string> = {
  '': 'all',
  'new': 'new',
  'ready': 'ready',
  'in-implementation': 'in-implementation',
  'in-review': 'in-review',
  'done': 'done',
  'cancelled': 'cancelled',
};

function candidateWords(title: string | undefined): string[] {
  if (!title) {
    return [];
  }

  return title
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((word) => word.length >= 4);
}

async function fetchTickets(
  page: Page,
  query?: string,
  state?: string,
): Promise<TicketListItem[]> {
  const params = new URLSearchParams({
    workspace: 'default',
    limit: '200',
  });
  if (query) {
    params.set('query', query);
  }
  if (state) {
    params.set('state', state);
  }

  const response = await page.request.get(
    `${TICKET_VIEWER_URL}/api/tickets?${params.toString()}`,
  );
  expect(response.ok(), `ticket list query failed for ${params.toString()}`).toBe(true);
  const body = await response.json();
  return body?.items ?? [];
}

async function findQueryStateCandidate(page: Page): Promise<QueryStateCandidate> {
  const items = await fetchTickets(page);
  for (const item of items) {
    if (!item.state || !(item.state in STATE_CHIP_BY_VALUE)) {
      continue;
    }

    for (const word of candidateWords(item.title)) {
      const filtered = await fetchTickets(page, word, item.state);
      if (filtered.some((candidate) => candidate.id === item.id)) {
        return {
          query: word,
          state: item.state,
        };
      }
    }
  }

  throw new Error('No query/state combination could be resolved for the current workspace');
}

test.describe('ticket-viewer — sidebar query + state filter', () => {
  test('sidebar explorer keeps active state filter when filter text is non-empty', async ({ page }) => {
    test.setTimeout(120_000);

    const candidate = await findQueryStateCandidate(page);

    await page.goto(`${TICKET_VIEWER_URL}/workspace/default`, {
      waitUntil: 'domcontentloaded',
    });
    await page.locator(TICKET_VIEWER.readySelector).first().waitFor({
      state: 'visible',
      timeout: TICKET_VIEWER.readyTimeout,
    });

    const filterInput = page.getByTestId('ticket-tree-filter');
    await expect(filterInput).toBeVisible();
    await filterInput.fill(candidate.query);

    const chipId = STATE_CHIP_BY_VALUE[candidate.state];
    const chip = page.getByTestId(`ticket-tree-state-chip-${chipId}`);
    await expect(chip).toBeVisible();
    const combinedResponsePromise = page.waitForResponse((response) => {
      if (!response.ok()) {
        return false;
      }

      const url = new URL(response.url());
      return (
        url.pathname === '/api/tickets' &&
        url.searchParams.get('workspace') === 'default' &&
        url.searchParams.get('query') === candidate.query &&
        url.searchParams.get('state') === candidate.state
      );
    });
    await chip.click();
    const combinedResponse = await combinedResponsePromise;
    const combinedBody = await combinedResponse.json();
    const expectedIds = (combinedBody?.items ?? []).map((item: TicketListItem) => item.id);
    const expectedStates = (combinedBody?.items ?? []).map(
      (item: TicketListItem) => item.state ?? 'new',
    );

    await expect.poll(async () => {
      const ids = await page.locator('button[data-testid^="ticket-tree-ticket-"]').evaluateAll((nodes) =>
        nodes
          .map((node) => node.getAttribute('data-testid') ?? '')
          .map((testId) => testId.replace('ticket-tree-ticket-', '')),
      );
      return ids.sort();
    }).toEqual([...expectedIds].sort());

    const rowStates = await page.locator('[data-testid^="ticket-tree-ticket-state-"]').evaluateAll((nodes) =>
      nodes.map((node) => node.textContent?.trim() ?? ''),
    );

    expect(expectedIds.length).toBeGreaterThan(0);
    expect(rowStates.length).toBeGreaterThan(0);
    expect([...rowStates].sort()).toEqual([...expectedStates].sort());
  });
});