import type {
  DocListResponse,
  DocContent,
  CreateDocRequest,
  CreateDocResponse,
  UpdateMetaRequest,
  SearchQuery,
  SearchResultsResponse,
  ValidationResponse,
  HealthDashboardResponse,
} from './types';
import { withSession } from '@context-engine/viewer-api-frontend';

const API_BASE = '/api';

// === Error Handling ===

class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public statusText: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: response.statusText }));
    throw new ApiError(
      error.error || response.statusText,
      response.status,
      response.statusText
    );
  }
  return response.json();
}

// === Docs API ===

/**
 * List all documentation, optionally filtered by type or tag
 */
export async function fetchDocs(options?: {
  docType?: string;
  tag?: string;
}): Promise<DocListResponse> {
  const params = new URLSearchParams();
  if (options?.docType) params.set('doc_type', options.docType);
  if (options?.tag) params.set('tag', options.tag);

  const url = params.toString()
    ? `${API_BASE}/docs?${params}`
    : `${API_BASE}/docs`;

  const response = await fetch(url, withSession());
  return handleResponse<DocListResponse>(response);
}

/**
 * Fetch a specific document by filename
 */
export async function fetchDoc(
  filename: string,
  detail: 'outline' | 'summary' | 'full' = 'full'
): Promise<DocContent> {
  const params = new URLSearchParams({ detail });
  const response = await fetch(
    `${API_BASE}/docs/${encodeURIComponent(filename)}?${params}`,
    withSession()
  );
  return handleResponse<DocContent>(response);
}

/**
 * Create a new document
 */
export async function createDoc(data: CreateDocRequest): Promise<CreateDocResponse> {
  const response = await fetch(`${API_BASE}/docs`, withSession({
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  }));
  return handleResponse<CreateDocResponse>(response);
}

/**
 * Update document metadata
 */
export async function updateDocMeta(
  filename: string,
  data: UpdateMetaRequest
): Promise<DocContent> {
  const response = await fetch(`${API_BASE}/docs/${encodeURIComponent(filename)}`, withSession({
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  }));
  return handleResponse<DocContent>(response);
}

/**
 * Search documents
 */
export async function searchDocs(query: SearchQuery): Promise<SearchResultsResponse> {
  const params = new URLSearchParams();
  if (query.query) params.set('query', query.query);
  if (query.tag) params.set('tag', query.tag);
  if (query.docType) params.set('doc_type', query.docType);
  if (query.searchContent !== undefined) params.set('search_content', String(query.searchContent));
  if (query.linesBefore !== undefined) params.set('lines_before', String(query.linesBefore));
  if (query.linesAfter !== undefined) params.set('lines_after', String(query.linesAfter));

  const response = await fetch(`${API_BASE}/docs/search?${params}`, withSession());
  return handleResponse<SearchResultsResponse>(response);
}

/**
 * Validate all documents
 */
export async function validateDocs(): Promise<ValidationResponse> {
  const response = await fetch(`${API_BASE}/docs/validate`, withSession());
  return handleResponse<ValidationResponse>(response);
}

/**
 * Get health dashboard
 */
export async function getHealthDashboard(): Promise<HealthDashboardResponse> {
  const response = await fetch(`${API_BASE}/docs/health`, withSession());
  return handleResponse<HealthDashboardResponse>(response);
}

// === Crates API ===

export interface CrateSummary {
  name: string;
  version: string | null;
  description: string;
  module_count: number;
  has_readme: boolean;
}

export interface CrateListResponse {
  crates: CrateSummary[];
}

export interface ModuleNode {
  name: string;
  path: string;
  description: string;
  has_readme: boolean;
  children: ModuleNode[];
  source_files: SourceFileLink[];
}

export interface CrateTreeResponse {
  name: string;
  description: string;
  children: ModuleNode[];
  source_files: SourceFileLink[];
  crate_path: string;
}

/**
 * List all documented crates
 */
export async function fetchCrates(): Promise<CrateListResponse> {
  const response = await fetch(`${API_BASE}/crates`, withSession());
  return handleResponse<CrateListResponse>(response);
}

/**
 * Browse a crate's module tree
 */
export async function browseCrate(name: string): Promise<CrateTreeResponse> {
  const response = await fetch(`${API_BASE}/crates/${encodeURIComponent(name)}`, withSession());
  return handleResponse<CrateTreeResponse>(response);
}

export interface CrateDocResponse {
  crate_name: string;
  module_path: string | null;
  index_yaml: string;
  readme: string | null;
  crate_path: string;
  source_files: SourceFileLink[];
}

export interface SourceFileLink {
  rel_path: string;
  abs_path: string;
  vscode_uri: string;
}

/**
 * Read crate or module documentation
 */
export async function fetchCrateDoc(
  crateName: string,
  modulePath?: string,
  includeReadme = true
): Promise<CrateDocResponse> {
  const params = new URLSearchParams();
  if (modulePath) params.set('module', modulePath);
  if (!includeReadme) params.set('include_readme', 'false');

  const url = params.toString()
    ? `${API_BASE}/crates/${encodeURIComponent(crateName)}/doc?${params}`
    : `${API_BASE}/crates/${encodeURIComponent(crateName)}/doc`;

  const response = await fetch(url, withSession());
  return handleResponse<CrateDocResponse>(response);
}

// === JQ Query API ===

export interface JqQueryRequest {
  jq: string;
  doc_type?: string;
  transform?: boolean;
  include_content?: boolean;
}

export interface JqQueryResponse {
  query: string;
  total: number;
  results: unknown[];
}

/**
 * Query documents using JQ expressions
 */
export async function queryDocs(request: JqQueryRequest): Promise<JqQueryResponse> {
  const response = await fetch(`${API_BASE}/query`, withSession({
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  }));
  return handleResponse<JqQueryResponse>(response);
}

/**
 * Fetch a document's markdown AST
 */
export async function fetchDocAst(filename: string): Promise<unknown> {
  const response = await fetch(`${API_BASE}/docs/${encodeURIComponent(filename)}/ast`, withSession());
  return handleResponse<unknown>(response);
}

// === Source File API ===

export interface SourceFileContent {
  path: string;
  content: string;
  language: string;
  total_lines: number;
}

/**
 * Fetch source file content
 */
export async function fetchSourceFile(absPath: string): Promise<SourceFileContent> {
  // Convert backslashes to forward slashes and don't encode the path
  // The path needs to remain a valid filesystem path for the backend
  const normalizedPath = absPath.replace(/\\/g, '/');
  const response = await fetch(`${API_BASE}/source/${normalizedPath}`, withSession());
  return handleResponse<SourceFileContent>(response);
}
