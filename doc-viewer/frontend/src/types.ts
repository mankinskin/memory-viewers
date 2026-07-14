// === Document Types ===

export interface DocSummary {
  filename: string;
  title: string;
  date: string;
  summary: string;
  tags: string[];
  status: string | null;
}

export interface Category {
  category: string;
  count: number;
  docs: DocSummary[];
}

export interface DocListResponse {
  total: number;
  categories: Category[];
}

export interface DocContent {
  filename: string;
  doc_type: string;
  title: string;
  date: string;
  summary: string;
  tags: string[];
  status: string | null;
  body: string | null;
}

// === Tree View Types ===

export type TreeNodeType = 'root' | 'category' | 'doc' | 'crate' | 'module' | 'file';

export interface SourceFileLink {
  rel_path: string;
  abs_path: string;
  vscode_uri: string;
}

export interface TreeNode {
  id: string;
  label: string;
  type: TreeNodeType;
  category?: string;
  children?: TreeNode[];
  data?: DocSummary;
  // For crate/module nodes
  crateName?: string;
  modulePath?: string;
  hasReadme?: boolean;
  // For file nodes
  sourceFile?: SourceFileLink;
}

// === Tab Types ===

export interface OpenTab {
  filename: string;
  title: string;
  doc: DocContent | null;
  isLoading: boolean;
}

// === API Request Types ===

export interface CreateDocRequest {
  doc_type: string;
  name: string;
  title: string;
  summary: string;
  tags: string[];
  status?: string;
}

export interface CreateDocResponse {
  filename: string;
  path: string;
}

export interface UpdateMetaRequest {
  tags?: string[];
  summary?: string;
  status?: string;
}

export interface SearchQuery {
  query?: string;
  tag?: string;
  docType?: string;
  searchContent?: boolean;
  linesBefore?: number;
  linesAfter?: number;
}

export interface SearchResultsResponse {
  query: string;
  total_matches: number;
  files_searched: number;
  results: SearchMatch[];
}

export interface SearchMatch {
  filename: string;
  doc_type: string;
  match_count: number;
  excerpts: SearchExcerpt[];
}

export interface SearchExcerpt {
  line_number: number;
  line: string;
  context_before: string[];
  context_after: string[];
}

// === Validation Types ===

export interface ValidationResponse {
  documents_checked: number;
  errors: number;
  warnings: number;
  issues: ValidationIssue[];
}

export interface ValidationIssue {
  file: string;
  category: string;
  issue: string;
  severity: 'error' | 'warning';
}

// === Health Dashboard Types ===

export interface HealthDashboardResponse {
  total_documents: number;
  frontmatter_coverage: number;
  index_sync_rate: number;
  naming_compliance: number;
  categories: CategoryHealth[];
  recommendations: string[];
}

export interface CategoryHealth {
  name: string;
  doc_count: number;
  has_frontmatter: number;
  has_valid_name: number;
  in_index: number;
}

// === JQ Query Types ===

export interface JqQueryResult {
  doc_type: string;
  filename: string;
  title: string;
  date: string;
  summary: string;
  tags: string[];
  status: string | null;
  content?: unknown; // Markdown AST when include_content is true
}
