// Root application component.
//
// Implements the tri-pane layout from wireframes-v0.1.md:
//   Left pane:   Workspace picker + ticket tree
//   Center pane: Tabbed description.md / ticket.toml viewer
//   Right pane:  Dependency graph view

import { JSX } from 'preact';
import { useEffect, useState, useCallback } from 'preact/hooks';
import { Sidebar as SharedSidebar, ThemeSettings } from '@context-engine/viewer-api-frontend';
import { ResizeHandle } from '@context-engine/viewer-api-frontend';
import { WgpuOverlay, MINIMAL_SCHEMA } from '@context-engine/viewer-api-frontend';
import { themeSettingsStore } from './theme';
import { WorkspacePicker } from './components/WorkspacePicker';
import { TicketTree } from './components/TicketTree';
import { TicketContent } from './components/TicketContent';
import { DependencyGraph } from './components/DependencyGraph';
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
  const gpuSchema = { ...MINIMAL_SCHEMA, isActive3DView: () => true };
  const [mobileOpen, setMobileOpen] = useState(false);
  const [rightPaneWidth, setRightPaneWidth] = useState(300);
  const [showTheme, setShowTheme] = useState(false);
  // Ensure theme store module is initialised on first render.
  void themeSettingsStore;

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
      <WgpuOverlay schema={gpuSchema} />
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
        <button
          class="header-icon-btn"
          onClick={() => setShowTheme((v) => !v)}
          aria-label="Theme settings"
          title="Theme settings"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="5" />
            <line x1="12" y1="1" x2="12" y2="3" />
            <line x1="12" y1="21" x2="12" y2="23" />
            <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
            <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
            <line x1="1" y1="12" x2="3" y2="12" />
            <line x1="21" y1="12" x2="23" y2="12" />
            <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
            <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
          </svg>
        </button>
      </header>

      {showTheme && (
        <div class="theme-overlay" onClick={(e) => { if (e.target === e.currentTarget) setShowTheme(false); }}>
          <div class="theme-overlay__panel">
            <ThemeSettings store={themeSettingsStore} />
          </div>
        </div>
      )}

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
              <DependencyGraph />
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
