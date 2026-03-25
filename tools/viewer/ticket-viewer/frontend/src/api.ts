// Typed fetch wrappers for the ticket serve REST API.
// All endpoints are relative so they work both via the reverse-proxy in prod
// and directly with Vite's dev server proxy.

import type {
  EdgesResponse,
    SubgraphResponse,
  TicketDescriptionResponse,
  TicketDetailResponse,
  TicketsResponse,
  WorkspacesResponse,
} from './types';

async function apiFetch<T>(path: string, token?: string): Promise<T> {
  const headers: Record<string, string> = { 'Accept': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  const res = await fetch(path, { headers });
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`${res.status} ${path}: ${text}`);
  }
  return res.json() as Promise<T>;
}

export function listWorkspaces(token?: string): Promise<WorkspacesResponse> {
  return apiFetch<WorkspacesResponse>('/api/workspaces', token);
}

export function listTickets(
  workspace: string,
  opts: { state?: string; query?: string; limit?: number } = {},
  token?: string,
): Promise<TicketsResponse> {
  const params = new URLSearchParams({ workspace });
  if (opts.state) params.set('state', opts.state);
  if (opts.query) params.set('query', opts.query);
  if (opts.limit !== undefined) params.set('limit', String(opts.limit));
  return apiFetch<TicketsResponse>(`/api/tickets?${params}`, token);
}

export function getTicket(
  id: string,
  workspace: string,
  token?: string,
): Promise<TicketDetailResponse> {
  return apiFetch<TicketDetailResponse>(
    `/api/tickets/${id}?workspace=${encodeURIComponent(workspace)}`,
    token,
  );
}

export function getTicketDescription(
  id: string,
  workspace: string,
  token?: string,
): Promise<TicketDescriptionResponse> {
  return apiFetch<TicketDescriptionResponse>(
    `/api/tickets/${id}/description?workspace=${encodeURIComponent(workspace)}`,
    token,
  );
}

export function listEdges(
  workspace: string,
  token?: string,
): Promise<EdgesResponse> {
  return apiFetch<EdgesResponse>(
    `/api/edges?workspace=${encodeURIComponent(workspace)}`,
    token,
  );
}

export function getSubgraph(
    workspace: string,
    root: string,
    opts: { direction?: 'both' | 'in' | 'out'; edge_kind?: string; depth?: number } = {},
    token?: string,
): Promise<SubgraphResponse> {
    const params = new URLSearchParams({ workspace, root });
    if (opts.direction) params.set('direction', opts.direction);
    if (opts.edge_kind) params.set('edge_kind', opts.edge_kind);
    if (opts.depth !== undefined) params.set('depth', String(opts.depth));
    return apiFetch<SubgraphResponse>(`/api/graph/subgraph?${params}`, token);
}
