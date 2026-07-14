import { signal, computed, createUrlStateManager } from '@context-engine/viewer-api-frontend';
import type { Category, TreeNode, OpenTab, DocContent, JqQueryResult, SourceFileLink } from './types';
import { fetchDocs, fetchCrates, queryDocs, fetchSourceFile, type ModuleNode, type CrateDocResponse } from './api';
import {
  getCachedDoc,
  getCachedCrateDoc,
  getCachedCrateTree,
  preloadSiblingDocs,
  preloadModuleNeighbors,
  preloadCrateRoots,
  preloadCategoryDocs,
} from './cache';

// State signals
export const categories = signal<Category[]>([]);
export const totalDocs = signal(0);
export const isLoading = signal(false);
export const error = signal<string | null>(null);

// Code viewer state signals
export const codeViewerFile = signal<string | null>(null);
export const codeViewerContent = signal<string>('');
export const codeViewerLine = signal<number | null>(null);
export const codeViewerLoading = signal<boolean>(false);

// Filter state signals
export const showFilterPanel = signal(false);
export const docTypeFilter = signal<string>('');
export const tagFilter = signal<string>('');
export const dateFromFilter = signal<string>('');
export const dateToFilter = signal<string>('');
export const jqFilter = signal<string>('');
export const jqResults = signal<JqQueryResult[] | null>(null);
export const isFilterLoading = signal(false);

// Tab state
export const openTabs = signal<OpenTab[]>([]);
export const activeTabId = signal<string | null>(null);

// Tree expansion state - tracks which node IDs are expanded
export const expandedNodes = signal<Set<string>>(new Set(['agents', 'crates']));

// Helper to expand all ancestors of a node ID
export function expandPathToNode(nodeId: string): void {
  const newExpanded = new Set(expandedNodes.value);

  // Always expand root nodes
  newExpanded.add('agents');
  newExpanded.add('crates');

  if (nodeId.startsWith('crate:')) {
    // For crate:name or crate:name:module/path
    const parts = nodeId.split(':');
    const crateName = parts[1];

    // Expand the crate node
    newExpanded.add(`crate:${crateName}`);

    // If there's a module path, expand parent modules
    if (parts.length > 2) {
      const modulePath = parts.slice(2).join(':');
      const segments = modulePath.split('/');
      // Expand each parent module level
      for (let i = 1; i < segments.length; i++) {
        const parentPath = segments.slice(0, i).join('/');
        newExpanded.add(`crate:${crateName}:${parentPath}`);
      }
    }
  } else if (!nodeId.startsWith('page:')) {
    // Agent doc - find its category and expand it
    const category = categories.value.find(c =>
      c.docs.some(d => d.filename === nodeId)
    );
    if (category) {
      newExpanded.add(`agent:${category.category}`);
    }
  }

  expandedNodes.value = newExpanded;
}

// Toggle expansion of a node
export function toggleNodeExpanded(nodeId: string): void {
  const newExpanded = new Set(expandedNodes.value);
  if (newExpanded.has(nodeId)) {
    newExpanded.delete(nodeId);
  } else {
    newExpanded.add(nodeId);
  }
  expandedNodes.value = newExpanded;
}

// URL routing - sync active document with URL hash
// Uses human-readable paths like #/crate/context-insert/module/path

// Convert internal filename to readable URL path
function filenameToUrlPath(filename: string): string {
  if (filename.startsWith('page:')) {
    return '/' + filename.slice(5);
  }
  if (filename.startsWith('crate:')) {
    const parts = filename.slice(6).split(':');
    return '/crate/' + parts.join('/');
  }
  return '/doc/' + filename;
}

// Convert URL path back to internal filename
function urlPathToFilename(urlPath: string): string | null {
  if (!urlPath || urlPath === '/') return null;
  
  const path = urlPath.startsWith('/') ? urlPath.slice(1) : urlPath;
  
  if (path === 'home' || path === 'agent-docs' || path === 'crate-docs') {
    return 'page:' + path;
  }
  if (path.startsWith('crate/')) {
    const parts = path.slice(6).split('/');
    if (parts.length === 1) return 'crate:' + parts[0];
    return `crate:${parts[0]}:${parts.slice(1).join('/')}`;
  }
  if (path.startsWith('doc/')) {
    return path.slice(4);
  }
  return path;
}

const urlState = createUrlStateManager<string>({
  stateToHash(filename) {
    return filenameToUrlPath(filename);
  },
  hashToState(hash) {
    // Backwards compatibility: decode URL-encoded hashes and old format
    let path = hash;
    try {
      const decoded = decodeURIComponent(hash);
      if (decoded.startsWith('/') || decoded.startsWith('page:') || decoded.startsWith('crate:')) {
        path = decoded;
      }
    } catch { /* keep original */ }

    if (path.startsWith('/')) return urlPathToFilename(path);
    // Old format: crate:name:path
    return path || null;
  },
  async onNavigate(filename) {
    const existingTab = openTabs.value.find(t => t.filename === filename);
    if (existingTab) {
      activeTabId.value = filename;
      expandPathToNode(filename);
    } else {
      await openDocFromPath(filename);
    }
  },
  getCurrentState() {
    return activeTabId.value;
  },
});

function updateUrlHash(filename: string | null): void {
  if (filename) {
    urlState.updateHash(filename);
  }
}

export function getDocFromUrl(): string | null {
  return urlState.getStateFromUrl();
}

export function initUrlListener(): void {
  urlState.initListener();
}

// Computed: currently active document
export const activeDoc = computed(() => {
  const tab = openTabs.value.find(t => t.filename === activeTabId.value);
  return tab?.doc ?? null;
});

// Computed: is the active tab loading?
export const isActiveTabLoading = computed(() => {
  const tab = openTabs.value.find(t => t.filename === activeTabId.value);
  return tab?.isLoading ?? false;
});

// Build tree structure from categories
export const docTree = signal<TreeNode[]>([]);

function buildAgentDocsTree(cats: Category[]): TreeNode {
  return {
    id: 'agents',
    label: 'Agent Docs',
    type: 'root',
    children: cats.map(cat => ({
      id: `agent:${cat.category}`,
      label: formatCategoryName(cat.category),
      type: 'category' as const,
      category: cat.category,
      children: cat.docs.map(doc => ({
        id: doc.filename,
        label: doc.title || doc.filename,
        type: 'doc' as const,
        category: cat.category,
        data: doc,
      })),
    })),
  };
}

function buildModuleTree(modules: ModuleNode[], crateName: string): TreeNode[] {
  return modules.flatMap(mod => {
    // Build module node
    const moduleNode: TreeNode = {
      id: `crate:${crateName}:${mod.path}`,
      label: mod.name,
      type: 'module' as const,
      crateName,
      modulePath: mod.path,
      hasReadme: mod.has_readme,
      children: mod.children?.length > 0
        ? buildModuleTree(mod.children, crateName)
        : undefined,
    };

    // Build file nodes from source_files if present
    const fileNodes = mod.source_files?.length > 0
      ? buildFileNodes(mod.source_files, crateName, mod.path)
      : [];

    // Add file nodes as children to the module node
    if (fileNodes.length > 0) {
      moduleNode.children = [...(moduleNode.children || []), ...fileNodes];
    }

    return [moduleNode];
  });
}

// Build file nodes from source files (filter out directories)
function buildFileNodes(sourceFiles: SourceFileLink[], crateName: string, modulePath?: string): TreeNode[] {
  return sourceFiles
    .filter(sf => !sf.rel_path.endsWith('/')) // Only include actual files, not directories
    .map(sf => ({
      id: `file:${sf.abs_path}`,
      label: sf.rel_path.split('/').pop() || sf.rel_path,
      type: 'file' as const,
      crateName,
      modulePath,
      sourceFile: sf,
    }));
}

// Update tree node children with file nodes
function addFileNodesToTree(sourceFiles: SourceFileLink[], crateName: string, modulePath?: string): void {
  if (sourceFiles.length === 0) return;

  const fileNodes = buildFileNodes(sourceFiles, crateName, modulePath);
  const nodeId = modulePath ? `crate:${crateName}:${modulePath}` : `crate:${crateName}`;

  docTree.value = docTree.value.map(root => {
    if (root.id !== 'crates') return root;
    return updateNodeChildren(root, nodeId, fileNodes);
  });

  // Expand the node so files are visible
  const newExpanded = new Set(expandedNodes.value);
  newExpanded.add(nodeId);
  expandedNodes.value = newExpanded;
}

function updateNodeChildren(node: TreeNode, targetId: string, fileNodes: TreeNode[]): TreeNode {
  if (node.id === targetId) {
    // Found the target node - add file nodes as children
    const existingChildren = node.children?.filter(c => c.type !== 'file') || [];
    return {
      ...node,
      children: [...existingChildren, ...fileNodes],
    };
  }

  if (!node.children) return node;

  return {
    ...node,
    children: node.children.map(child => updateNodeChildren(child, targetId, fileNodes)),
  };
}

function formatCategoryName(name: string): string {
  // Convert kebab-case to Title Case
  return name
    .split('-')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

// Actions
export async function loadDocs(): Promise<void> {
  isLoading.value = true;
  error.value = null;
  
  try {
    // Load both agent docs and crates in parallel
    const [docsData, cratesData] = await Promise.all([
      fetchDocs(),
      fetchCrates(),
    ]);

    categories.value = docsData.categories;
    totalDocs.value = docsData.total;

    // Build unified tree with roots for Agent Docs and Crates
    const tree: TreeNode[] = [];

    // Add Agent Docs root
    if (docsData.categories.length > 0) {
      tree.push(buildAgentDocsTree(docsData.categories));
    }

    // Add Crates root with lazy-loaded children
    if (cratesData.crates.length > 0) {
      tree.push({
        id: 'crates',
        label: 'Crate Docs',
        type: 'root',
        children: cratesData.crates.map(crate => ({
          id: `crate:${crate.name}`,
          label: crate.name,
          type: 'crate' as const,
          crateName: crate.name,
          hasReadme: crate.has_readme,
          // Children will be loaded when expanded
          children: [],
        })),
      });
    }

    docTree.value = tree;

    // Preload initial docs in background
    preloadCategoryDocs(docsData.categories);
    preloadCrateRoots(cratesData.crates.map(c => c.name));
    
    // Preload crate trees for initially visible crates (root is auto-expanded)
    preloadVisibleCrateTrees(cratesData.crates.map(c => c.name));

    // Open document from URL or default to home page
    if (openTabs.value.length === 0) {
      const urlPath = getDocFromUrl();
      await openDocFromPath(urlPath || 'page:home');
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load docs';
  } finally {
    isLoading.value = false;
  }
}

// Preload crate module trees for visible crates and update docTree with children counts
export function preloadVisibleCrateTrees(crateNames: string[]): void {
  // Load in background without blocking
  setTimeout(async () => {
    for (const crateName of crateNames) {
      try {
        const tree = await getCachedCrateTree(crateName);

        // Build file nodes from crate-level source files
        const crateFileNodes = tree.source_files?.length > 0
          ? buildFileNodes(tree.source_files, crateName)
          : [];

        // Build module children tree
        const moduleChildren = buildModuleTree(tree.children, crateName);

        // Update the crate node with loaded module tree
        docTree.value = docTree.value.map(root => {
          if (root.id !== 'crates') return root;
          return {
            ...root,
            children: root.children?.map(node => {
              if (node.crateName !== crateName) return node;
              // Only update if not already loaded
              if (node.children && node.children.length > 0) return node;
              return {
                ...node,
                children: [...moduleChildren, ...crateFileNodes],
              };
            }),
          };
        });
      } catch {
        // Silently ignore failures during preload
      }
    }
  }, 100);
}

// Load crate modules when crate is expanded
export async function loadCrateModules(crateName: string): Promise<void> {
  try {
    const tree = await getCachedCrateTree(crateName);

    // Build file nodes from crate-level source files
    const crateFileNodes = tree.source_files?.length > 0
      ? buildFileNodes(tree.source_files, crateName)
      : [];

    // Build module children tree
    const moduleChildren = buildModuleTree(tree.children, crateName);

    // Update the crate node with loaded module tree and crate-level source files
    docTree.value = docTree.value.map(root => {
      if (root.id !== 'crates') return root;
      return {
        ...root,
        children: root.children?.map(node => {
          if (node.crateName !== crateName) return node;
          return {
            ...node,
            children: [...moduleChildren, ...crateFileNodes],
          };
        }),
      };
    });

    // Preload root-level module docs
    preloadModuleNeighbors(crateName, undefined, tree.children);
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load crate modules';
  }
}

// Convert crate doc response to DocContent format for tab display
function crateDocToContent(doc: CrateDocResponse): DocContent {
  const title = doc.module_path
    ? `${doc.crate_name}::${doc.module_path.replace(/\//g, '::')}`
    : doc.crate_name;

  // Combine index.yaml and readme into body
  let body = '## index.yaml\n\n```yaml\n' + doc.index_yaml + '\n```\n';
  if (doc.readme) {
    body += '\n## README\n\n' + doc.readme;
  }

  return {
    filename: `crate:${doc.crate_name}${doc.module_path ? ':' + doc.module_path : ''}`,
    doc_type: 'crate',
    title,
    date: '',
    summary: '',
    tags: [],
    status: null,
    body,
  };
}

export async function openDoc(filename: string, title: string): Promise<void> {
  // Check if already open
  const existingIndex = openTabs.value.findIndex(t => t.filename === filename);
  if (existingIndex >= 0) {
    // Just switch to this tab
    activeTabId.value = filename;
    updateUrlHash(filename);
    return;
  }

  // Add new tab with loading state
  const newTab: OpenTab = {
    filename,
    title,
    doc: null,
    isLoading: true,
  };

  openTabs.value = [...openTabs.value, newTab];
  activeTabId.value = filename;
  updateUrlHash(filename);
  
  try {
    const doc = await getCachedDoc(filename);
    // Update the tab with loaded content
    openTabs.value = openTabs.value.map(t =>
      t.filename === filename
        ? { ...t, doc, isLoading: false }
        : t
    );

    // Preload sibling documents from the same category
    const category = categories.value.find(c => c.docs.some(d => d.filename === filename));
    if (category) {
      preloadSiblingDocs(category, filename);
    }
  } catch (e) {
    // Update tab to show error state
    openTabs.value = openTabs.value.map(t =>
      t.filename === filename
        ? { ...t, isLoading: false }
        : t
    );
    error.value = e instanceof Error ? e.message : 'Failed to load document';
  }
}

// Open crate or module documentation
export async function openCrateDoc(crateName: string, modulePath?: string): Promise<void> {
  const filename = `crate:${crateName}${modulePath ? ':' + modulePath : ''}`;
  const title = modulePath
    ? `${crateName}::${modulePath.replace(/\//g, '::')}`
    : crateName;

  // Check if already open
  const existingIndex = openTabs.value.findIndex(t => t.filename === filename);
  if (existingIndex >= 0) {
    activeTabId.value = filename;
    updateUrlHash(filename);
    return;
  }

  // Add new tab with loading state
  const newTab: OpenTab = {
    filename,
    title,
    doc: null,
    isLoading: true,
  };

  openTabs.value = [...openTabs.value, newTab];
  activeTabId.value = filename;
  updateUrlHash(filename);

  try {
    const doc = await getCachedCrateDoc(crateName, modulePath);
    const content = crateDocToContent(doc);
    openTabs.value = openTabs.value.map(t =>
      t.filename === filename
        ? { ...t, doc: content, isLoading: false }
        : t
    );

    // Add file nodes to the tree for this module's source files
    if (doc.source_files && doc.source_files.length > 0) {
      addFileNodesToTree(doc.source_files, crateName, modulePath);
    }

    // Preload module neighbors - need to get the crate tree first
    const crateNode = docTree.value
      .find(n => n.id === 'crates')?.children
      ?.find(n => n.crateName === crateName);
    if (crateNode?.children && crateNode.children.length > 0) {
      preloadModuleNeighbors(crateName, modulePath, crateNode.children as unknown as ModuleNode[]);
    }
  } catch (e) {
    openTabs.value = openTabs.value.map(t =>
      t.filename === filename
        ? { ...t, isLoading: false }
        : t
    );
    error.value = e instanceof Error ? e.message : 'Failed to load crate documentation';
  }
}

export function closeTab(filename: string): void {
  const tabs = openTabs.value;
  const index = tabs.findIndex(t => t.filename === filename);
  if (index < 0) return;

  // Remove the tab
  const newTabs = tabs.filter(t => t.filename !== filename);
  openTabs.value = newTabs;

  // If this was the active tab, switch to another
  if (activeTabId.value === filename) {
    let newActiveId: string | null = null;
    if (newTabs.length === 0) {
      newActiveId = null;
    } else if (index >= newTabs.length) {
      // Closed last tab, select previous
      newActiveId = newTabs[newTabs.length - 1].filename;
    } else {
      // Select the tab at same position
      newActiveId = newTabs[index].filename;
    }
    activeTabId.value = newActiveId;
    updateUrlHash(newActiveId);
  }
}

export function setActiveTab(filename: string): void {
  activeTabId.value = filename;
  updateUrlHash(filename);
}

// Legacy: for backwards compatibility with Sidebar
export const selectedFilename = computed(() => activeTabId.value);

export function selectDoc(filename: string): void {
  // Find the doc info from tree - need to search recursively now
  function findInTree(nodes: TreeNode[]): TreeNode | undefined {
    for (const node of nodes) {
      if (node.id === filename) return node;
      if (node.children) {
        const found = findInTree(node.children);
        if (found) return found;
      }
    }
    return undefined;
  }

  const found = findInTree(docTree.value);
  if (found) {
    openDoc(filename, found.label);
  } else {
    // Fallback: use filename as title
    openDoc(filename, filename);
  }
}

// === Filter Functions ===

// Compute all unique tags from loaded documents
export const allTags = computed(() => {
  const tags = new Set<string>();
  for (const cat of categories.value) {
    for (const doc of cat.docs) {
      for (const tag of doc.tags ?? []) {
        tags.add(tag);
      }
    }
  }
  return Array.from(tags).sort();
});

// Compute all doc types from loaded documents
export const allDocTypes = computed(() => {
  return categories.value.map(cat => cat.category);
});

// Build JQ query from current filters
export function buildJqQuery(): string {
  const conditions: string[] = [];

  if (docTypeFilter.value) {
    conditions.push(`(.doc_type == "${docTypeFilter.value}")`);
  }
  if (tagFilter.value) {
    conditions.push(`(.tags | any(. == "${tagFilter.value}"))`);
  }
  if (dateFromFilter.value) {
    conditions.push(`(.date >= "${dateFromFilter.value}")`);
  }
  if (dateToFilter.value) {
    conditions.push(`(.date <= "${dateToFilter.value}")`);
  }

  if (conditions.length === 0) {
    return '.'; // Identity - return all
  }

  return `select(${conditions.join(' and ')})`;
}

// Execute JQ query
export async function executeJqQuery(customQuery?: string): Promise<void> {
  const query = customQuery || buildJqQuery();

  if (query === '.' && !customQuery) {
    // No filters applied, clear results
    jqResults.value = null;
    jqFilter.value = '';
    return;
  }

  isFilterLoading.value = true;
  jqFilter.value = query;

  try {
    const response = await queryDocs({
      jq: query,
      doc_type: docTypeFilter.value || undefined,
    });
    jqResults.value = response.results as JqQueryResult[];
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Query failed';
    jqResults.value = null;
  } finally {
    isFilterLoading.value = false;
  }
}

// Clear all filters
export function clearFilters(): void {
  docTypeFilter.value = '';
  tagFilter.value = '';
  dateFromFilter.value = '';
  dateToFilter.value = '';
  jqFilter.value = '';
  jqResults.value = null;
}

// Check if any filters are active
export const hasActiveFilters = computed(() => {
  return !!(
    docTypeFilter.value ||
    tagFilter.value ||
    dateFromFilter.value ||
    dateToFilter.value ||
    jqFilter.value
  );
});

// Open a category/navigation page (Home, Agent Docs, Crate Docs)
export function openCategoryPage(pageId: string): void {
  const titles: Record<string, string> = {
    'page:home': 'Home',
    'page:agent-docs': 'Agent Docs',
    'page:crate-docs': 'Crate Docs',
  };

  const title = titles[pageId] || pageId;

  // Check if already open
  const existingIndex = openTabs.value.findIndex(t => t.filename === pageId);
  if (existingIndex >= 0) {
    activeTabId.value = pageId;
    updateUrlHash(pageId);
    return;
  }

  // Add new tab (no loading needed for category pages)
  openTabs.value = [...openTabs.value, {
    filename: pageId,
    title,
    doc: {
      filename: pageId,
      doc_type: 'category',
      title,
      date: '',
      summary: '',
      tags: [],
      status: null,
      body: null,
    },
    isLoading: false,
  }];
  activeTabId.value = pageId;
  updateUrlHash(pageId);
}

// Open document from URL path (used on initial load)
export async function openDocFromPath(path: string): Promise<void> {
  if (!path) {
    openCategoryPage('page:home');
    return;
  }

  // Category pages
  if (path.startsWith('page:')) {
    openCategoryPage(path);
    return;
  }

  // Expand tree to show the document
  expandPathToNode(path);

  // Crate documentation
  if (path.startsWith('crate:')) {
    const parts = path.split(':');
    const crateName = parts[1];
    const modulePath = parts.slice(2).join(':') || undefined;
    await openCrateDoc(crateName, modulePath);
    return;
  }

  // Agent documentation - try to find title from tree
  const node = findDocInTree(path);
  const title = node?.label || path;
  await openDoc(path, title);
}

function findDocInTree(filename: string): TreeNode | undefined {
  function search(nodes: TreeNode[]): TreeNode | undefined {
    for (const node of nodes) {
      if (node.id === filename) return node;
      if (node.children) {
        const found = search(node.children);
        if (found) return found;
      }
    }
    return undefined;
  }
  return search(docTree.value);
}

// Open a source file in the code viewer
export async function openSourceFile(sourceFile: SourceFileLink): Promise<void> {
  codeViewerLoading.value = true;
  codeViewerFile.value = sourceFile.rel_path;

  try {
    const result = await fetchSourceFile(sourceFile.abs_path);
    codeViewerContent.value = result.content;
    codeViewerLine.value = null;
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load source file';
    codeViewerContent.value = '';
  } finally {
    codeViewerLoading.value = false;
  }
}

// Close the code viewer
export function closeCodeViewer(): void {
  codeViewerFile.value = null;
  codeViewerContent.value = '';
  codeViewerLine.value = null;
}
