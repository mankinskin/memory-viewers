import { expect, test, type Page } from '@playwright/test';
import { TICKET_VIEWER } from './shared/viewers';

const TICKET_VIEWER_URL = process.env.TICKET_VIEWER_URL ?? TICKET_VIEWER.url;

interface TicketListItem {
  id: string;
  title?: string;
}

function candidateWords(title: string | undefined): string[] {
  if (!title) {
    return [];
  }

  return title
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((word) => word.length >= 4);
}

async function gotoWorkspace(page: Page): Promise<void> {
  await page.addInitScript(() => {
    localStorage.removeItem('ticket-viewer:default:ui');
  });
  await page.goto(`${TICKET_VIEWER_URL}/workspace/default`, {
    waitUntil: 'domcontentloaded',
  });
  await page.locator(TICKET_VIEWER.readySelector).first().waitFor({
    state: 'visible',
    timeout: TICKET_VIEWER.readyTimeout,
  });
}

async function visibleSidebarTicketIds(page: Page): Promise<string[]> {
  return page
    .locator('button[data-testid^="ticket-tree-ticket-"]')
    .evaluateAll((nodes) =>
      nodes
        .map((node) => node.getAttribute('data-testid') ?? '')
        .map((testId) => testId.replace('ticket-tree-ticket-', '')),
    );
}

async function activeSidebarTicketId(page: Page): Promise<string | null> {
  const active = page.locator('button[data-testid^="ticket-tree-ticket-"][aria-selected="true"]').first();
  if ((await active.count()) === 0) {
    return null;
  }

  const testId = await active.getAttribute('data-testid');
  return testId?.replace('ticket-tree-ticket-', '') ?? null;
}

async function visibleSearchResultIds(page: Page): Promise<string[]> {
  return page
    .locator('button[data-testid^="search-result-"]')
    .evaluateAll((nodes) =>
      nodes
        .map((node) => node.getAttribute('data-testid') ?? '')
        .map((testId) => testId.replace('search-result-', '')),
    );
}

async function activeSearchResultId(page: Page): Promise<string | null> {
  const active = page.locator('button[data-testid^="search-result-"][aria-selected="true"]').first();
  if ((await active.count()) === 0) {
    return null;
  }

  const testId = await active.getAttribute('data-testid');
  return testId?.replace('search-result-', '') ?? null;
}

async function fetchTickets(
  page: Page,
  query?: string,
): Promise<TicketListItem[]> {
  const params = new URLSearchParams({
    workspace: 'default',
    limit: '200',
  });
  if (query) {
    params.set('query', query);
  }

  const response = await page.request.get(
    `${TICKET_VIEWER_URL}/api/tickets?${params.toString()}`,
  );
  expect(response.ok(), `ticket list query failed for ${params.toString()}`).toBe(true);
  const body = await response.json();
  return body?.items ?? [];
}

async function findSearchCandidate(page: Page): Promise<{ query: string; ids: string[] }> {
  const items = await fetchTickets(page);
  const words = new Set<string>();
  for (const item of items) {
    for (const word of candidateWords(item.title)) {
      words.add(word);
    }
  }

  for (const word of words) {
    const matches = await fetchTickets(page, word);
    if (matches.length >= 2) {
      return {
        query: word,
        ids: matches.slice(0, 2).map((item) => item.id),
      };
    }
  }

  throw new Error('No quick-search query with at least two results was found');
}

async function findBroadSearchCandidate(
  page: Page,
  minimumResults: number,
): Promise<{ query: string; ids: string[] }> {
  const items = await fetchTickets(page);
  const words = new Set<string>();
  for (const item of items) {
    for (const word of candidateWords(item.title)) {
      words.add(word);
    }
  }

  for (const word of words) {
    const matches = await fetchTickets(page, word);
    if (matches.length >= minimumResults) {
      return {
        query: word,
        ids: matches.slice(0, minimumResults).map((item) => item.id),
      };
    }
  }

  throw new Error(`No quick-search query with at least ${minimumResults} results was found`);
}

async function findSubstringSearchCandidate(
  page: Page,
): Promise<{ query: string; expectedId: string }> {
  const items = await fetchTickets(page);
  for (const item of items) {
    for (const word of candidateWords(item.title)) {
      if (word.length < 8) {
        continue;
      }

      const substring = word.slice(2, word.length - 1);
      const matches = await fetchTickets(page, substring);
      if (matches.some((candidate) => candidate.id === item.id)) {
        return {
          query: substring,
          expectedId: item.id,
        };
      }
    }
  }

  throw new Error('No quick-search substring candidate could be resolved for the current workspace');
}

test.describe('ticket-viewer — keyboard navigation', () => {
  test('sidebar filter keeps focus while arrows move the active ticket and Enter selects it', async ({ page }) => {
    await gotoWorkspace(page);

    await expect.poll(async () => (await visibleSidebarTicketIds(page)).length).toBeGreaterThan(1);

    const filterInput = page.getByTestId('ticket-tree-filter');
    const filterHint = page.getByTestId('ticket-tree-filter-hint');
    await expect(filterHint).toContainText('Free text searches titles and descriptions');
    await expect(filterHint).toContainText('id:<value>');
    await expect(filterHint).not.toContainText('priority:');
    await filterInput.click();
    await expect(filterInput).toBeFocused();

    const visibleIds = await visibleSidebarTicketIds(page);
    await expect.poll(async () => activeSidebarTicketId(page)).not.toBeNull();
    const currentActiveId = await activeSidebarTicketId(page);
    expect(currentActiveId).toBeTruthy();

    const currentIndex = visibleIds.findIndex((id) => id === currentActiveId);
    expect(currentIndex).toBeGreaterThanOrEqual(0);

    const nextId = visibleIds[Math.min(currentIndex + 1, visibleIds.length - 1)];
    expect(nextId).toBeTruthy();

    await page.keyboard.press('ArrowDown');
    await expect(page.getByTestId(`ticket-tree-ticket-${nextId}`)).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await expect(filterInput).toBeFocused();

    await page.keyboard.press('ArrowUp');
    await expect(page.getByTestId(`ticket-tree-ticket-${currentActiveId}`)).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await expect(filterInput).toBeFocused();

    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    await expect(page.getByTestId(`ticket-tree-ticket-${nextId}`)).toHaveAttribute(
      'data-selected',
      'true',
    );
    await expect(filterInput).toBeFocused();
  });

  test('quick-search keeps input focus while arrows move the active result and Enter opens it', async ({ page }) => {
    const candidate = await findSearchCandidate(page);
    await gotoWorkspace(page);

    await page.locator('body').click();
    await page.keyboard.press('/');

    const searchInput = page.getByTestId('search-input');
    await expect(searchInput).toBeVisible();
    await expect(searchInput).toBeFocused();
    await searchInput.fill(candidate.query);

    await expect.poll(async () => (await visibleSearchResultIds(page)).length).toBeGreaterThan(1);

    const visibleIds = await visibleSearchResultIds(page);
    await expect.poll(async () => activeSearchResultId(page)).not.toBeNull();
    const currentActiveId = await activeSearchResultId(page);
    expect(currentActiveId).toBeTruthy();

    const currentIndex = visibleIds.findIndex((id) => id === currentActiveId);
    expect(currentIndex).toBeGreaterThanOrEqual(0);

    const nextId = visibleIds[Math.min(currentIndex + 1, visibleIds.length - 1)];
    expect(nextId).toBeTruthy();

    const currentResult = page.getByTestId(`search-result-${currentActiveId}`);
    const nextResult = page.getByTestId(`search-result-${nextId}`);
    await expect(currentResult).toHaveAttribute('aria-selected', 'true');

    await page.keyboard.press('ArrowDown');
    await expect(nextResult).toHaveAttribute('aria-selected', 'true');
    await expect(searchInput).toBeFocused();

    await page.keyboard.press('ArrowUp');
    await expect(currentResult).toHaveAttribute('aria-selected', 'true');
    await expect(searchInput).toBeFocused();

    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');

    await expect(page.getByTestId('search-input')).toHaveCount(0);
    await expect(page.getByTestId(`ticket-tree-ticket-${nextId}`)).toHaveAttribute(
      'data-selected',
      'true',
    );
  });

  test('quick-search documents supported syntax and id predicates narrow results', async ({ page }) => {
    const items = await fetchTickets(page);
    expect(items.length).toBeGreaterThan(0);

    const candidate = items[0];
    const titleCandidate = await findSubstringSearchCandidate(page);
    await gotoWorkspace(page);

    await page.locator('body').click();
    await page.keyboard.press('/');

    const searchInput = page.getByTestId('search-input');
    const syntaxHint = page.getByTestId('search-syntax-hint');
    await expect(searchInput).toBeVisible();
    await expect(searchInput).toHaveAttribute('placeholder', /id:, title:, state:, type:/);
    await expect(syntaxHint).toContainText('Free text searches titles and descriptions');
    await expect(syntaxHint).toContainText('state:<value>/status:<value>');
    await expect(syntaxHint).toContainText('type:<value>/ticket_type:<value>');
    await expect(syntaxHint).not.toContainText('priority:');

    await searchInput.fill(`id:${candidate.id}`);

    await expect.poll(async () => visibleSearchResultIds(page)).toEqual([candidate.id]);

    await searchInput.fill(`title:${titleCandidate.query}`);

    await expect.poll(async () =>
      (await visibleSearchResultIds(page)).includes(titleCandidate.expectedId),
    ).toBe(true);
  });

  test('quick-search surfaces at least nine results for broad queries', async ({ page }) => {
    const candidate = await findBroadSearchCandidate(page, 9);
    await gotoWorkspace(page);

    await page.locator('body').click();
    await page.keyboard.press('/');

    const searchInput = page.getByTestId('search-input');
    await expect(searchInput).toBeVisible();
    await searchInput.fill(candidate.query);

    await expect.poll(async () => (await visibleSearchResultIds(page)).length).toBeGreaterThanOrEqual(
      candidate.ids.length,
    );
    await expect.poll(async () => (await visibleSearchResultIds(page)).slice(0, candidate.ids.length)).toEqual(
      candidate.ids,
    );
  });

  test('quick-search matches partial-word substrings', async ({ page }) => {
    const candidate = await findSubstringSearchCandidate(page);
    await gotoWorkspace(page);

    await page.locator('body').click();
    await page.keyboard.press('/');

    const searchInput = page.getByTestId('search-input');
    await expect(searchInput).toBeVisible();
    await searchInput.fill(candidate.query);

    await expect.poll(async () =>
      (await visibleSearchResultIds(page)).includes(candidate.expectedId),
    ).toBe(true);
  });
});