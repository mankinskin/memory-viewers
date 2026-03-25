// Root application component.
//
// Implements the tri-pane layout from wireframes-v0.1.md:
//   Left pane:   Workspace picker + ticket tree
//   Center pane: Tabbed description.md / ticket.toml viewer
//   Right pane:  Dependency graph view

import { JSX } from 'preact';
import { useEffect, useState, useCallback } from 'preact/hooks';
import { Sidebar as SharedSidebar } from '@context-engine/viewer-api-frontend';
import { ResizeHandle } from '@context-engine/viewer-api-frontend';
import { WorkspacePicker } from './components/WorkspacePicker';
import { TicketTree } from './components/TicketTree';
import { TicketContent } from './components/TicketContent';
import { GraphView } from './components/GraphView';
import {
  authToken,
  globalError,
  filteredTickets,
  selectedWorkspace,
  workspaces,
  workspacesLoading,
  tickets,
  ticketsLoading,
  restoreWorkspaceState,
} from './store';
import { listWorkspaces, listTickets } from './api';

export function App(): JSX.Element {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [rightPaneWidth, setRightPaneWidth] = useState(300);

  // Load workspace list on mount.
  useEffect(() => {
    async function load() {
      workspacesLoading.value = true;
      try {
        const resp = await listWorkspaces(authToken.value || undefined);
        workspaces.value = resp.workspaces;

        // Auto-select first workspace if none saved.
        if (!selectedWorkspace.value && resp.workspaces.length > 0) {
          const first = resp.workspaces[0].name;
          selectedWorkspace.value = first;
          restoreWorkspaceState(first);

          ticketsLoading.value = true;
          try {
            const ticketResp = await listTickets(
              first,
              {},
              authToken.value || undefined,
            );
            tickets.value = ticketResp.items;
          } finally {
            ticketsLoading.value = false;
          }
        }
      } catch (e) {
        globalError.value = String(e);
      } finally {
        workspacesLoading.value = false;
      }
    }
    void load();
  }, []);

  const toggleMobileSidebar = useCallback(() => {
    setMobileOpen((prev) => !prev);
  }, []);

  const closeMobileSidebar = useCallback(() => {
    setMobileOpen(false);
  }, []);

  const resizeRightPane = useCallback((delta: number) => {
    setRightPaneWidth((prev) => Math.max(0, prev + delta));
  }, []);

  const error = globalError.value;

  return (
    <div class="app ticket-viewer-app">
      <header class="header app-header">
        <button
          class="sidebar-hamburger"
          onClick={toggleMobileSidebar}
          aria-label="Toggle sidebar"
          title="Toggle sidebar"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <line x1="3" y1="6" x2="21" y2="6" />
            <line x1="3" y1="12" x2="21" y2="12" />
            <line x1="3" y1="18" x2="21" y2="18" />
          </svg>
        </button>
        <span class="app-header__title">Ticket Viewer</span>
        <span class="app-header__workspace">
          {selectedWorkspace.value || ''}
        </span>
      </header>

      {error && (
        <div class="global-error-banner" role="alert">
          <strong>Error:</strong> {error}
          <button
            class="global-error-banner__dismiss"
            onClick={() => (globalError.value = null)}
            aria-label="Dismiss error"
          >
            ×
          </button>
        </div>
      )}

      <div class="main-layout">
        <SharedSidebar
          class="ticket-sidebar"
          title="Tickets"
          badge={filteredTickets.value.length}
          collapsible
          resizable
          initialWidth={300}
          mobileOpen={mobileOpen}
          onMobileClose={closeMobileSidebar}
        >
          <div class="left-pane">
            <WorkspacePicker />
            <TicketTree />
          </div>
        </SharedSidebar>

        <main class="content">
          {/* Center + right pane inside a split container */}
          <div class="center-right-split">
            <div class="center-pane">
              <TicketContent />
            </div>
            <div class="right-pane" style={{ width: `${rightPaneWidth}px` }}>
              <ResizeHandle
                direction="horizontal"
                edge="left"
                deltaSign={-1}
                onResize={resizeRightPane}
              />
              <GraphView />
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
