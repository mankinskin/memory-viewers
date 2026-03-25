// TicketContent: tabbed panel showing description.md and ticket fields (toml-
// equivalent) for the currently selected ticket.

import { JSX } from 'preact';
import { marked } from 'marked';
import hljs from 'highlight.js';
import { markedHighlight } from 'marked-highlight';
import {
  activeTab,
  detailError,
  detailLoading,
  openTicketDescription,
  openTicketDetail,
  openTicketId,
} from '../store';
import type { TabId } from '../types';

// Configure marked once with syntax highlighting.
marked.use(
  markedHighlight({
    langPrefix: 'hljs language-',
    highlight(code, lang) {
      const language = hljs.getLanguage(lang) ? lang : 'plaintext';
      return hljs.highlight(code, { language }).value;
    },
  }),
);

const TABS: { id: TabId; label: string }[] = [
  { id: 'description', label: 'description.md' },
  { id: 'fields', label: 'ticket.toml' },
];

export function TicketContent(): JSX.Element {
  const id = openTicketId.value;

  if (!id) {
    return (
      <div class="ticket-content ticket-content--empty">
        <p>Select a ticket from the tree to view its contents.</p>
      </div>
    );
  }

  if (detailLoading.value) {
    return <div class="ticket-content ticket-content--loading">Loading…</div>;
  }

  if (detailError.value) {
    return (
      <div class="ticket-content ticket-content--error">
        <strong>Error:</strong> {detailError.value}
      </div>
    );
  }

  return (
    <div class="ticket-content">
      {/* Tab bar */}
      <div class="ticket-content__tabs" role="tablist">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            role="tab"
            class={[
              'ticket-content__tab',
              activeTab.value === tab.id ? 'ticket-content__tab--active' : '',
            ].join(' ')}
            aria-selected={activeTab.value === tab.id}
            onClick={() => (activeTab.value = tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab panels */}
      <div class="ticket-content__body">
        {activeTab.value === 'description' && (
          <DescriptionPanel description={openTicketDescription.value} />
        )}
        {activeTab.value === 'fields' && (
          <FieldsPanel fields={openTicketDetail.value?.fields ?? {}} id={id} />
        )}
      </div>
    </div>
  );
}

// ── Description panel ─────────────────────────────────────────────────────────

interface DescriptionPanelProps {
  description: string | null;
}

function DescriptionPanel({ description }: DescriptionPanelProps): JSX.Element {
  if (description === null) {
    return (
      <div class="ticket-content__placeholder">
        <em>No description.md found for this ticket.</em>
      </div>
    );
  }

  const html = marked.parse(description) as string;

  return (
    <div
      class="ticket-content__markdown markdown-body"
      // Safe: content comes from trusted internal ticket files.
      // eslint-disable-next-line react/no-danger
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

// ── Fields panel (ticket.toml equivalent) ────────────────────────────────────

interface FieldsPanelProps {
  fields: Record<string, unknown>;
  id: string;
}

function FieldsPanel({ fields, id }: FieldsPanelProps): JSX.Element {
  const toml = fieldsToToml(id, fields);

  return (
    <pre class="ticket-content__toml">
      <code class="language-toml">{toml}</code>
    </pre>
  );
}

/** Convert the fields object to a TOML-like textual representation. */
function fieldsToToml(id: string, fields: Record<string, unknown>): string {
  const lines: string[] = [`# ${id}`, ''];
  for (const [k, v] of Object.entries(fields)) {
    const val = typeof v === 'string' ? `"${v.replace(/"/g, '\\"')}"` : String(v);
    lines.push(`${k} = ${val}`);
  }
  return lines.join('\n');
}
