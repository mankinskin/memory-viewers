// TicketTree: workspace-aware ticket list rendered as a tree via viewer-api's
// TreeView.  Supports filtering by state and a text search over title/id.

import { JSX } from 'preact';
import {
  FileTree,
  type TreeNode,
  type SortOption,
  type SortState,
} from '@context-engine/viewer-api-frontend';
import {
  activeTab,
  authToken,
  detailError,
  detailLoading,
  filteredTickets,
  globalError,
  openTicketDescription,
  openTicketDetail,
  openTicketId,
  selectedWorkspace,
  ticketsLoading,
  treeFilter,
  treeStateFilter,
  treeSortState,
} from '../store';
import { getTicket, getTicketDescription } from '../api';
import type { TicketSummary } from '../types';

const STATE_BADGE_COLORS: Record<string, string> = {
  open: '#4a9eff',
  'in-progress': '#f0a500',
  review: '#9b7fe8',
  validating: '#63b3ed',
  validated: '#48bb78',
  done: '#68d391',
  blocked: '#fc8181',
  cancelled: '#a0aec0',
};

const TICKET_SORT_OPTIONS: SortOption[] = [
  { key: 'title', label: 'Title', defaultDirection: 'asc' },
  { key: 'updated_at', label: 'Modified', defaultDirection: 'desc' },
  { key: 'created_at', label: 'Created', defaultDirection: 'desc' },
];

const stateOrder = [
  'open', 'in-progress', 'review', 'validating', 'validated',
  'done', 'blocked', 'cancelled', 'unknown',
];

function stateBadge(state: string | null): string | undefined {
  if (!state) return undefined;
  return state;
}

function sortTickets(tickets: TicketSummary[], sort: SortState): TicketSummary[] {
  return [...tickets].sort((a, b) => {
    let cmp = 0;
    if (sort.key === 'title') {
      const aTitle = a.title ?? '';
      const bTitle = b.title ?? '';
      cmp = aTitle.localeCompare(bTitle);
    } else if (sort.key === 'updated_at') {
      cmp = a.updated_at.localeCompare(b.updated_at);
    } else if (sort.key === 'created_at') {
      cmp = a.created_at.localeCompare(b.created_at);
    }
    return sort.direction === 'asc' ? cmp : -cmp;
  });
}

/** Convert flat ticket list to tree nodes grouped by state, sorted within groups. */
function buildTree(tickets: TicketSummary[], sort: SortState): TreeNode<TicketSummary>[] {
  // Group by state.
  const groups = new Map<string, TicketSummary[]>();
  for (const t of tickets) {
    const key = t.state ?? 'unknown';
    const list = groups.get(key) ?? [];
    list.push(t);
    groups.set(key, list);
  }

  const orderedKeys = [
    ...stateOrder.filter((s) => groups.has(s)),
    ...[...groups.keys()].filter((k) => !stateOrder.includes(k)),
  ];

  return orderedKeys.map((state) => {
    const group = sortTickets(groups.get(state)!, sort);
    return {
      id: `group:${state}`,
      label: state,
      icon: 'folder',
      badge: group.length,
      children: group.map((t) => ({
        id: t.id,
        label: t.title ?? t.id.slice(0, 8),
        icon: 'file',
        badge: stateBadge(t.state),
        data: t,
        tooltip: t.id,
      })),
    };
  });
}

async function openTicket(id: string) {
  if (openTicketId.value === id) return;
  openTicketId.value = id;
  openTicketDetail.value = null;
  openTicketDescription.value = null;
  detailError.value = null;
  detailLoading.value = true;
  activeTab.value = 'description';
  const ws = selectedWorkspace.value;
  const token = authToken.value || undefined;

  try {
    const [detailResp, descResp] = await Promise.all([
      getTicket(id, ws, token),
      getTicketDescription(id, ws, token),
    ]);
    openTicketDetail.value = detailResp.ticket;
    openTicketDescription.value = descResp.description;
  } catch (e) {
    detailError.value = String(e);
    globalError.value = String(e);
  } finally {
    detailLoading.value = false;
  }
}

export function TicketTree(): JSX.Element {
  const isLoading = ticketsLoading.value;
  const list = filteredTickets.value;
  const sort = treeSortState.value;
  const nodes = buildTree(list, sort);

  return (
    <div class="ticket-tree">
      {/* Search + filter toolbar */}
      <div class="ticket-tree__toolbar">
        <input
          type="search"
          class="ticket-tree__search"
          placeholder="Search tickets…"
          value={treeFilter.value}
          onInput={(e) =>
            (treeFilter.value = (e.target as HTMLInputElement).value)
          }
        />
        <select
          class="ticket-tree__state-filter"
          value={treeStateFilter.value}
          onChange={(e) =>
            (treeStateFilter.value = (e.target as HTMLSelectElement).value)
          }
        >
          <option value="">All states</option>
          {Object.keys(STATE_BADGE_COLORS).map((s) => (
            <option key={s} value={s}>
              {s}
            </option>
          ))}
        </select>
      </div>

      <FileTree<TicketSummary>
        class="ticket-tree__tree"
        nodes={nodes}
        selectedId={openTicketId.value ?? undefined}
        defaultExpanded={nodes.map((n) => n.id)}
        loading={isLoading}
        emptyMessage={
          selectedWorkspace.value
            ? 'No tickets match the current filter.'
            : 'Select a workspace to begin.'
        }
        sortOptions={TICKET_SORT_OPTIONS}
        sortState={sort}
        onSortChange={(s: SortState) => { treeSortState.value = s; }}
        onSelect={(node: TreeNode<TicketSummary>) => {
          // Only leaf nodes (tickets) are selectable; group folders are ignored.
          if (!node.id.startsWith('group:')) {
            void openTicket(node.id);
          }
        }}
      />
    </div>
  );
}

