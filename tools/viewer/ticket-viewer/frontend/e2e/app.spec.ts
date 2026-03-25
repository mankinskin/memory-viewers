/**
 * End-to-end tests for the ticket-viewer SPA.
 *
 * All API calls are intercepted with `page.route()` — no real `ticket serve`
 * backend is required.  The Vite dev server is started automatically by the
 * Playwright `webServer` config.
 */

import { test, expect, type Page, type Route } from '@playwright/test';

// ── Shared mock data ──────────────────────────────────────────────────────────

const TICKET_ID = 'aaaa1111-0000-0000-0000-000000000001';
const TICKET_ID_2 = 'bbbb2222-0000-0000-0000-000000000002';
const WORKSPACE = 'test-workspace';

const MOCK_WORKSPACES = {
  request_id: 'r1',
  workspaces: [{ name: WORKSPACE }],
};

const MOCK_TICKETS = {
  request_id: 'r2',
  workspace: WORKSPACE,
  items: [
    {
      id: TICKET_ID,
      type: 'tracker-improvement',
      title: 'First open ticket',
      state: 'open',
      updated_at: '2026-01-01T00:00:00Z',
      fields: { component: 'ui' },
    },
    {
      id: TICKET_ID_2,
      type: 'tracker-improvement',
      title: 'A done ticket',
      state: 'done',
      updated_at: '2026-01-02T00:00:00Z',
      fields: { component: 'backend' },
    },
  ],
  next_cursor: null,
};

const MOCK_TICKET_DETAIL = {
  request_id: 'r3',
  workspace: WORKSPACE,
  ticket: {
    id: TICKET_ID,
    created_at: '2026-01-01T00:00:00Z',
    fields: {
      title: 'First open ticket',
      state: 'open',
      component: 'ui',
      acceptance_criteria: 'It works',
    },
  },
};

const MOCK_DESCRIPTION = {
  request_id: 'r4',
  workspace: WORKSPACE,
  id: TICKET_ID,
  description: '# First open ticket\n\nThis ticket does something **useful**.',
};

const MOCK_EDGES = {
  request_id: 'r5',
  workspace: WORKSPACE,
  edges: [],
};

const MOCK_SUBGRAPH = {
    request_id: 'r6',
    workspace: WORKSPACE,
    nodes: [
        {
            id: TICKET_ID,
            title: 'First open ticket',
            state: 'open',
            depth: 0,
        },
        {
            id: TICKET_ID_2,
            title: 'A done ticket',
            state: 'done',
            depth: 1,
        },
    ],
    edges: [
        {
            from: TICKET_ID,
            to: TICKET_ID_2,
            kind: 'depends_on',
        },
    ],
    truncated: false,
    next_cursor: null,
    stats: {
        nodes_returned: 2,
        edges_returned: 1,
        max_depth_reached: 1,
    },
};

// ── Route helpers ─────────────────────────────────────────────────────────────

/** Intercept all ticket-serve API calls with mock responses. */
async function mockApi(page: Page): Promise<void> {
  await page.route('**/api/workspaces**', (route: Route) => {
    void route.fulfill({ json: MOCK_WORKSPACES });
  });
  // Single handler dispatches to list / detail / description based on path.
  await page.route('**/api/tickets**', (route: Route) => {
    const { pathname } = new URL(route.request().url());
    const afterTickets = pathname.split('/api/tickets')[1] ?? '';
    if (afterTickets.includes('/description')) {
      void route.fulfill({ json: MOCK_DESCRIPTION });
    } else if (afterTickets.startsWith('/') && afterTickets.length > 1) {
      // /api/tickets/{id}
      void route.fulfill({ json: MOCK_TICKET_DETAIL });
    } else {
      // /api/tickets or /api/tickets?...
      void route.fulfill({ json: MOCK_TICKETS });
    }
  });
  await page.route('**/api/edges**', (route: Route) => {
    void route.fulfill({ json: MOCK_EDGES });
  });
    await page.route('**/api/graph/subgraph**', (route: Route) => {
        void route.fulfill({ json: MOCK_SUBGRAPH });
    });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

test.describe('ticket-viewer app', () => {
  test.beforeEach(async ({ page }) => {
    await mockApi(page);
    await page.goto('/');
  });

  test('renders the tri-pane layout shell', async ({ page }) => {
    // Root app div is present.
    await expect(page.locator('.ticket-viewer-app')).toBeVisible();
    // Header shows title.
    await expect(page.locator('.app-header__title')).toHaveText('Ticket Viewer');
    // Left pane is present.
    await expect(page.locator('.left-pane')).toBeVisible();
    // Center + right split is present.
    await expect(page.locator('.center-right-split')).toBeVisible();
    await expect(page.locator('.center-pane')).toBeVisible();
    await expect(page.locator('.right-pane')).toBeVisible();
  });

  test('workspace picker renders and shows loaded workspace', async ({
    page,
  }) => {
    const select = page.locator('#ws-select');
    await expect(select).toBeVisible();

    // After load, the first workspace is auto-selected.
    await expect(select).toHaveValue(WORKSPACE);

    // The option label is visible in the dropdown.
    await expect(select.locator(`option[value="${WORKSPACE}"]`)).toHaveText(
      WORKSPACE,
    );
  });

  test('ticket tree shows grouped tickets after workspace loads', async ({
    page,
  }) => {
    await expect(page.locator('.ticket-tree')).toBeVisible();

    // State group labels — exact text match via regex.
    await expect(
      page.locator('.tree-label').filter({ hasText: /^open$/ }).first(),
    ).toBeVisible();
    await expect(
      page.locator('.tree-label').filter({ hasText: /^done$/ }).first(),
    ).toBeVisible();

    // Individual ticket nodes.
    await expect(
      page.locator('.tree-label', { hasText: 'First open ticket' }),
    ).toBeVisible();
    await expect(
      page.locator('.tree-label', { hasText: 'A done ticket' }),
    ).toBeVisible();
  });

  test('tree toggle and document icons stay compact', async ({ page }) => {
    const toggleSvg = page.locator('.tree-toggle svg').first();
    const iconSvg = page.locator('.tree-icon svg').first();

    await expect(toggleSvg).toBeVisible();
    await expect(iconSvg).toBeVisible();

    const toggleBox = await toggleSvg.boundingBox();
    const iconBox = await iconSvg.boundingBox();

    expect(toggleBox).not.toBeNull();
    expect(iconBox).not.toBeNull();

    if (!toggleBox || !iconBox) {
      return;
    }

    // Guard against browser fallback intrinsic SVG size (300x150).
    expect(toggleBox.width).toBeLessThanOrEqual(16);
    expect(toggleBox.height).toBeLessThanOrEqual(16);
    expect(iconBox.width).toBeLessThanOrEqual(20);
    expect(iconBox.height).toBeLessThanOrEqual(20);

    // Re-check after filtering to guard against layout/style regressions in
    // dynamically re-rendered tree rows.
    await page.locator('.ticket-tree__search').fill('done ticket');

    const filteredToggleSvg = page.locator('.tree-toggle svg').first();
    const filteredIconSvg = page.locator('.tree-icon svg').first();
    await expect(filteredToggleSvg).toBeVisible();
    await expect(filteredIconSvg).toBeVisible();

    const filteredToggleBox = await filteredToggleSvg.boundingBox();
    const filteredIconBox = await filteredIconSvg.boundingBox();

    expect(filteredToggleBox).not.toBeNull();
    expect(filteredIconBox).not.toBeNull();

    if (!filteredToggleBox || !filteredIconBox) {
      return;
    }

    expect(filteredToggleBox.width).toBeLessThanOrEqual(16);
    expect(filteredToggleBox.height).toBeLessThanOrEqual(16);
    expect(filteredIconBox.width).toBeLessThanOrEqual(20);
    expect(filteredIconBox.height).toBeLessThanOrEqual(20);
  });

  test('search filter narrows the ticket tree', async ({ page }) => {
    const search = page.locator('.ticket-tree__search');
    await expect(search).toBeVisible();

    // Type a query that matches only the "done" ticket.
    await search.fill('done ticket');

    // The done group and its ticket stay in the tree.
    await expect(
      page.locator('.tree-label').filter({ hasText: /^done$/ }),
    ).toBeVisible();
    await expect(
      page.locator('.tree-label', { hasText: 'A done ticket' }),
    ).toBeVisible();

    // The open group and its ticket are removed from the tree.
    await expect(
      page.locator('.tree-label').filter({ hasText: /^open$/ }),
    ).not.toBeVisible();
    await expect(
      page.locator('.tree-label', { hasText: 'First open ticket' }),
    ).not.toBeVisible();
  });

  test('clicking a ticket opens the content panel with description tab', async ({
    page,
  }) => {
    const ticketNode = page.locator('.tree-item-row').filter({
      has: page.locator('.tree-label', { hasText: 'First open ticket' }),
    });
    await expect(ticketNode).toBeVisible();
    await ticketNode.click();

    // Content panel switches from empty to loaded.
    const content = page.locator('.ticket-content');
    await expect(content).toBeVisible();
    await expect(content).not.toHaveClass(/ticket-content--empty/);

    // Description tab is active by default.
    const descTab = page.locator('[role="tab"]').filter({ hasText: 'description.md' });
    await expect(descTab).toHaveAttribute('aria-selected', 'true');

    // Rendered markdown is present.
    const markdown = page.locator('.ticket-content__markdown');
    await expect(markdown).toBeVisible();
    await expect(markdown.locator('h1')).toHaveText('First open ticket');
    await expect(markdown.locator('strong')).toHaveText('useful');
  });

  test('ticket.toml tab shows fields panel', async ({ page }) => {
    // Open a ticket first.
    await page
      .locator('.tree-item-row')
      .filter({ has: page.locator('.tree-label', { hasText: 'First open ticket' }) })
      .click();
    await expect(page.locator('.ticket-content')).toBeVisible();

    // Switch to fields tab.
    const fieldsTab = page.locator('[role="tab"]').filter({ hasText: 'ticket.toml' });
    await fieldsTab.click();

    await expect(fieldsTab).toHaveAttribute('aria-selected', 'true');

    // TOML pre block appears with field content.
    const toml = page.locator('.ticket-content__toml');
    await expect(toml).toBeVisible();
    await expect(toml).toContainText('component');
  });

  test('empty content panel shown when no ticket is selected', async ({
    page,
  }) => {
    // Nothing selected on load.
    const empty = page.locator('.ticket-content--empty');
    await expect(empty).toBeVisible();
    await expect(empty).toContainText('Select a ticket');
  });

    test('graph panel prompts selection when no ticket is selected', async ({
        page,
    }) => {
        const graph = page.locator('.graph-view');
        await expect(graph).toBeVisible();
        await expect(graph).toContainText('Select a ticket to explore its dependency graph.');
    });

    test('graph panel renders dependency nodes after selecting a ticket', async ({
        page,
    }) => {
        await page
            .locator('.tree-item-row')
            .filter({ has: page.locator('.tree-label', { hasText: 'First open ticket' }) })
            .click();

        await expect(page.locator('.graph-view__svg')).toBeVisible();
        await expect(page.locator('.graph-view__node-label')).toContainText([
            'First open ticket',
            'A done ticket',
        ]);
    });

    test('graph panel shows an error if subgraph request fails', async ({
        page,
    }) => {
        await page.unroute('**/api/graph/subgraph**');
        await page.route('**/api/graph/subgraph**', (route: Route) => {
            void route.fulfill({
                status: 500,
                contentType: 'text/plain',
                body: 'subgraph unavailable',
            });
        });

        await page
            .locator('.tree-item-row')
            .filter({ has: page.locator('.tree-label', { hasText: 'First open ticket' }) })
            .click();

        const error = page.locator('.graph-view__error');
        await expect(error).toBeVisible();
        await expect(error).toContainText('Failed to load graph');
  });

  test('token button toggles auth input', async ({ page }) => {
    const tokenBtn = page.locator('.workspace-picker__token-btn');
    await expect(tokenBtn).toBeVisible();

    // Token input hidden initially.
    await expect(
      page.locator('.workspace-picker__token-input'),
    ).not.toBeVisible();

    // Click to reveal.
    await tokenBtn.click();
    await expect(page.locator('.workspace-picker__token-input')).toBeVisible();

    // Click again to hide.
    await tokenBtn.click();
    await expect(
      page.locator('.workspace-picker__token-input'),
    ).not.toBeVisible();
  });
});
