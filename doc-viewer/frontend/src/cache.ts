// Document cache and preloading for doc-viewer
// Preloads reachable documents to minimize loading times

import { signal } from '@context-engine/viewer-api-frontend';
import type { DocContent, Category } from './types';
import { fetchDoc, fetchCrateDoc, browseCrate, type CrateDocResponse, type CrateTreeResponse, type ModuleNode } from './api';

// Cache storage
const docCache = new Map<string, DocContent>();
const crateDocCache = new Map<string, CrateDocResponse>();
const crateTreeCache = new Map<string, CrateTreeResponse>();

// Track pending fetches to avoid duplicate requests
const pendingFetches = new Map<string, Promise<unknown>>();

// Signal for cache stats (for debugging/monitoring)
export const cacheStats = signal({
  docHits: 0,
  docMisses: 0,
  preloadCount: 0,
});

/**
 * Get a document from cache or fetch it.
 * Returns synchronously if cached, otherwise returns a Promise.
 */
export function getCachedDoc(filename: string): DocContent | Promise<DocContent> {
    // Synchronous cache check
  const cached = docCache.get(filename);
  if (cached) {
    cacheStats.value = { ...cacheStats.value, docHits: cacheStats.value.docHits + 1 };
    return cached;
  }

  cacheStats.value = { ...cacheStats.value, docMisses: cacheStats.value.docMisses + 1 };
  
  // Check if already fetching
  const pending = pendingFetches.get(`doc:${filename}`);
  if (pending) {
    return pending as Promise<DocContent>;
  }

  // Fetch and cache
  const fetchPromise = fetchDoc(filename).then(doc => {
    docCache.set(filename, doc);
    pendingFetches.delete(`doc:${filename}`);
    return doc;
  });
  
  pendingFetches.set(`doc:${filename}`, fetchPromise);
  return fetchPromise;
}

/**
 * Get crate documentation from cache or fetch it.
 * Returns synchronously if cached, otherwise returns a Promise.
 */
export function getCachedCrateDoc(
  crateName: string,
  modulePath?: string
): CrateDocResponse | Promise<CrateDocResponse> {
  const cacheKey = `${crateName}:${modulePath || ''}`;

    // Synchronous cache check
  const cached = crateDocCache.get(cacheKey);
  if (cached) {
    cacheStats.value = { ...cacheStats.value, docHits: cacheStats.value.docHits + 1 };
    return cached;
  }

  cacheStats.value = { ...cacheStats.value, docMisses: cacheStats.value.docMisses + 1 };

  // Check if already fetching
  const pending = pendingFetches.get(`crate:${cacheKey}`);
  if (pending) {
    return pending as Promise<CrateDocResponse>;
  }

  // Fetch and cache
  const fetchPromise = fetchCrateDoc(crateName, modulePath).then(doc => {
    crateDocCache.set(cacheKey, doc);
    pendingFetches.delete(`crate:${cacheKey}`);
    return doc;
  });

  pendingFetches.set(`crate:${cacheKey}`, fetchPromise);
  return fetchPromise;
}

/**
 * Get crate tree from cache or fetch it.
 * Returns synchronously if cached, otherwise returns a Promise.
 */
export function getCachedCrateTree(crateName: string): CrateTreeResponse | Promise<CrateTreeResponse> {
    // Synchronous cache check
  const cached = crateTreeCache.get(crateName);
  if (cached) {
    return cached;
  }

  // Check if already fetching
  const pending = pendingFetches.get(`tree:${crateName}`);
  if (pending) {
    return pending as Promise<CrateTreeResponse>;
  }

  // Fetch and cache
  const fetchPromise = browseCrate(crateName).then(tree => {
    crateTreeCache.set(crateName, tree);
    pendingFetches.delete(`tree:${crateName}`);
    return tree;
  });

  pendingFetches.set(`tree:${crateName}`, fetchPromise);
  return fetchPromise;
}

/**
 * Preload documents in the background.
 * Does not block, silently fails on errors.
 */
function preloadInBackground(fetchers: (() => unknown)[]): void {
  // Run preloads with a slight delay to not compete with main content
  setTimeout(() => {
    for (const fetcher of fetchers) {
        // Handle both sync (cache hit) and async (fetch) returns
        Promise.resolve(fetcher()).catch(() => {
        // Silently ignore preload errors
      });
      cacheStats.value = { ...cacheStats.value, preloadCount: cacheStats.value.preloadCount + 1 };
    }
  }, 100);
}

/**
 * Preload sibling documents from the same category.
 * Call this when opening a doc to preload nearby docs.
 */
export function preloadSiblingDocs(category: Category, currentFilename: string): void {
  const currentIndex = category.docs.findIndex(d => d.filename === currentFilename);
  if (currentIndex < 0) return;

    const toPreload: (() => unknown)[] = [];

  // Preload 3 docs before and after current
  for (let i = Math.max(0, currentIndex - 3); i <= Math.min(category.docs.length - 1, currentIndex + 3); i++) {
    const doc = category.docs[i];
    if (doc.filename !== currentFilename && !docCache.has(doc.filename)) {
      toPreload.push(() => getCachedDoc(doc.filename));
    }
  }

  preloadInBackground(toPreload);
}

/**
 * Preload module siblings and children.
 * Call this when opening a module doc.
 */
export function preloadModuleNeighbors(
  crateName: string,
  modulePath: string | undefined,
  modules: ModuleNode[]
): void {
    const toPreload: (() => unknown)[] = [];

  // Find current module in the tree
  function findInTree(nodes: ModuleNode[], path: string | undefined): ModuleNode | undefined {
    for (const node of nodes) {
      if (node.path === path) return node;
        if (node.children?.length > 0) {
        const found = findInTree(node.children, path);
        if (found) return found;
      }
    }
    return undefined;
  }

  // Find parent nodes to get siblings
  function findParentOf(nodes: ModuleNode[], path: string | undefined, parent: ModuleNode | null = null): ModuleNode | null {
    for (const node of nodes) {
      if (node.path === path) return parent;
        if (node.children?.length > 0) {
        const found = findParentOf(node.children, path, node);
        if (found !== null) return found;
      }
    }
    return null;
  }

  const current = findInTree(modules, modulePath);
  const parent = findParentOf(modules, modulePath);

  // Preload children of current module
  if (current?.children) {
    for (const child of current.children.slice(0, 5)) {
      const cacheKey = `${crateName}:${child.path}`;
      if (!crateDocCache.has(cacheKey)) {
        toPreload.push(() => getCachedCrateDoc(crateName, child.path));
      }
    }
  }

  // Preload siblings (from parent's children)
  const siblings = parent?.children || modules;
  const currentIndex = siblings.findIndex(n => n.path === modulePath);
  if (currentIndex >= 0) {
    // Preload 2 siblings before and after
    for (let i = Math.max(0, currentIndex - 2); i <= Math.min(siblings.length - 1, currentIndex + 2); i++) {
      const sibling = siblings[i];
      if (sibling.path !== modulePath) {
        const cacheKey = `${crateName}:${sibling.path}`;
        if (!crateDocCache.has(cacheKey)) {
          toPreload.push(() => getCachedCrateDoc(crateName, sibling.path));
        }
      }
    }
  }

  preloadInBackground(toPreload);
}

/**
 * Preload crate root docs.
 * Call this when viewing the crates list.
 */
export function preloadCrateRoots(crateNames: string[]): void {
    const toPreload: (() => unknown)[] = [];

  // Preload first few crate root docs
  for (const name of crateNames.slice(0, 5)) {
    const cacheKey = `${name}:`;
    if (!crateDocCache.has(cacheKey)) {
      toPreload.push(() => getCachedCrateDoc(name));
    }
    // Also preload the tree structure
    if (!crateTreeCache.has(name)) {
      toPreload.push(() => getCachedCrateTree(name));
    }
  }

  preloadInBackground(toPreload);
}

/**
 * Preload category docs.
 * Call this when viewing the agent docs list.
 */
export function preloadCategoryDocs(categories: Category[]): void {
    const toPreload: (() => unknown)[] = [];

  // Preload first 2 docs from each category
  for (const cat of categories) {
    for (const doc of cat.docs.slice(0, 2)) {
      if (!docCache.has(doc.filename)) {
        toPreload.push(() => getCachedDoc(doc.filename));
      }
    }
  }

  preloadInBackground(toPreload);
}

/**
 * Clear all caches.
 * Useful for debugging or when data might be stale.
 */
export function clearCache(): void {
  docCache.clear();
  crateDocCache.clear();
  crateTreeCache.clear();
  pendingFetches.clear();
  cacheStats.value = { docHits: 0, docMisses: 0, preloadCount: 0 };
}

/**
 * Get cache size for debugging
 */
export function getCacheSize(): { docs: number; crateDocs: number; crateTrees: number } {
  return {
    docs: docCache.size,
    crateDocs: crateDocCache.size,
    crateTrees: crateTreeCache.size,
  };
}
