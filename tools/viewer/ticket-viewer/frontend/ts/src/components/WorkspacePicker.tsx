// WorkspacePicker: dropdown that lists available workspaces and switches the
// selected one, persisting and restoring per-workspace UI state.

import { JSX } from 'preact';
import { useSignal } from '@preact/signals';
import {
  authToken,
  globalError,
  openTicketDetail,
  openTicketId,
  persistWorkspaceState,
  restoreWorkspaceState,
  selectedWorkspace,
  tickets,
  ticketsLoading,
  workspaces,
} from '../store';
import { listTickets } from '../api';

export function WorkspacePicker(): JSX.Element {
  const showTokenInput = useSignal(false);
  const tokenDraft = useSignal(authToken.value);

  async function selectWorkspace(name: string) {
    if (name === selectedWorkspace.value) return;

    // Persist current state before switching.
    if (selectedWorkspace.value) {
      persistWorkspaceState(selectedWorkspace.value);
    }

    selectedWorkspace.value = name;
    openTicketId.value = null;
    openTicketDetail.value = null;
    tickets.value = [];

    if (!name) return;

    // Restore saved UI state for the new workspace.
    restoreWorkspaceState(name);

    // Load tickets.
    ticketsLoading.value = true;
    try {
      const resp = await listTickets(name, {}, authToken.value || undefined);
      tickets.value = resp.items;
    } catch (e) {
      globalError.value = String(e);
    } finally {
      ticketsLoading.value = false;
    }
  }

  function saveToken() {
    authToken.value = tokenDraft.value.trim();
    showTokenInput.value = false;
    // Reload ticket list with updated token.
    if (selectedWorkspace.value) {
      void selectWorkspace(selectedWorkspace.value);
    }
  }

  return (
    <div class="workspace-picker">
      <div class="workspace-picker__row">
        <label class="workspace-picker__label" for="ws-select">
          Workspace
        </label>
        <select
          id="ws-select"
          class="workspace-picker__select"
          value={selectedWorkspace.value}
          onChange={(e) =>
            void selectWorkspace((e.target as HTMLSelectElement).value)
          }
        >
          <option value="">— select —</option>
          {workspaces.value.map((ws) => (
            <option key={ws.name} value={ws.name}>
              {ws.name}
            </option>
          ))}
        </select>

        <button
          class="workspace-picker__token-btn"
          title="Configure auth token"
          onClick={() => {
            showTokenInput.value = !showTokenInput.value;
          }}
        >
          🔑
        </button>
      </div>

      {showTokenInput.value && (
        <div class="workspace-picker__token-row">
          <input
            type="password"
            class="workspace-picker__token-input"
            placeholder="Bearer token"
            value={tokenDraft.value}
            onInput={(e) =>
              (tokenDraft.value = (e.target as HTMLInputElement).value)
            }
            onKeyDown={(e) => e.key === 'Enter' && saveToken()}
          />
          <button class="workspace-picker__token-save" onClick={saveToken}>
            Save
          </button>
        </div>
      )}
    </div>
  );
}
