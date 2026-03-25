// Preact Signals store for the ticket viewer.
//
// State is organised into three layers:
//   1. Global: auth token, workspace list, selected workspace.
//   2. Tree: ticket list for the current workspace, filter/search.
//   3. Panel: the open ticket + active tab.

import { signal, computed } from '@preact/signals';
import type { SortState } from '@context-engine/viewer-api-frontend';
import type { TicketDetail, TicketSummary, TabId, WorkspaceInfo } from './types';

// ── Auth ──────────────────────────────────────────────────────────────────────

/** Bearer token read from sessionStorage on first load. */
export const authToken = signal<string>(
  sessionStorage.getItem('ticketViewerToken') ?? '',
);

authToken.subscribe((t) => {
  if (t) sessionStorage.setItem('ticketViewerToken', t);
  else sessionStorage.removeItem('ticketViewerToken');
});

// ── Workspaces ────────────────────────────────────────────────────────────────

export const workspaces = signal<WorkspaceInfo[]>([]);
export const selectedWorkspace = signal<string>('');

// ── Ticket tree ───────────────────────────────────────────────────────────────

/** Full ticket list for the current workspace. */
export const tickets = signal<TicketSummary[]>([]);

/** Current filter: state or search query. */
export const treeFilter = signal<string>('');
export const treeStateFilter = signal<string>('');

/** Current sort state for the ticket tree. */
export const treeSortState = signal<SortState>({ key: 'updated_at', direction: 'desc' });

export const filteredTickets = computed(() => {
  let list = tickets.value;
  const sf = treeStateFilter.value;
  const qf = treeFilter.value.toLowerCase();
  if (sf) list = list.filter((t) => t.state === sf);
  if (qf)
    list = list.filter(
      (t) =>
        t.title?.toLowerCase().includes(qf) ||
        t.id.toLowerCase().includes(qf),
    );
  return list;
});

// ── Open ticket panel ─────────────────────────────────────────────────────────

export const openTicketId = signal<string | null>(null);
export const openTicketDetail = signal<TicketDetail | null>(null);
export const openTicketDescription = signal<string | null>(null);
export const activeTab = signal<TabId>('description');

/** Loading / error flags for the detail panel. */
export const detailLoading = signal<boolean>(false);
export const detailError = signal<string | null>(null);

// ── Global loading / error ────────────────────────────────────────────────────

export const globalError = signal<string | null>(null);
export const workspacesLoading = signal<boolean>(false);
export const ticketsLoading = signal<boolean>(false);

// ── Workspace-scoped UI state persistence ─────────────────────────────────────

interface WorkspaceUIState {
  filter: string;
  stateFilter: string;
  sortState: SortState;
  openTicketId: string | null;
  activeTab: TabId;
}

const WS_STATE_KEY = 'ticketViewerWsState';

function loadWsState(ws: string): Partial<WorkspaceUIState> {
  try {
    const raw = localStorage.getItem(`${WS_STATE_KEY}:${ws}`);
    return raw ? (JSON.parse(raw) as Partial<WorkspaceUIState>) : {};
  } catch {
    return {};
  }
}

function saveWsState(ws: string, state: WorkspaceUIState) {
  try {
    localStorage.setItem(`${WS_STATE_KEY}:${ws}`, JSON.stringify(state));
  } catch {
    /* ignore quota errors */
  }
}

/** Restore saved per-workspace UI state. */
export function restoreWorkspaceState(ws: string) {
  const saved = loadWsState(ws);
  treeFilter.value = saved.filter ?? '';
  treeStateFilter.value = saved.stateFilter ?? '';
  treeSortState.value = saved.sortState ?? { key: 'updated_at', direction: 'desc' };
  openTicketId.value = saved.openTicketId ?? null;
  openTicketDetail.value = null;
  openTicketDescription.value = null;
  activeTab.value = saved.activeTab ?? 'description';
}

/** Persist current per-workspace UI state. */
export function persistWorkspaceState(ws: string) {
  saveWsState(ws, {
    filter: treeFilter.value,
    stateFilter: treeStateFilter.value,
    sortState: treeSortState.value,
    openTicketId: openTicketId.value,
    activeTab: activeTab.value,
  });
}
