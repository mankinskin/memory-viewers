import { useEffect, useMemo, useCallback } from '@context-engine/viewer-api-frontend';
import { FileTree, type TreeNode as SharedTreeNode } from '@context-engine/viewer-api-frontend';
import { h, type ComponentChildren } from 'preact';
import { docTree, isLoading, selectedFilename, selectDoc, loadCrateModules, openCrateDoc, openCategoryPage, preloadVisibleCrateTrees, expandedNodes, toggleNodeExpanded, openSourceFile, codeViewerFile } from '../store';
import type { TreeNode } from '../types';

export function Sidebar() {
  // Watch for tree changes and preload crate trees that aren't loaded yet
  useEffect(() => {
    const cratesRoot = docTree.value.find(n => n.id === 'crates');
    if (cratesRoot?.children) {
      const unloadedCrates = cratesRoot.children
        .filter(child => child.type === 'crate' && (!child.children || child.children.length === 0))
        .map(child => child.crateName!)
        .filter(Boolean);
      if (unloadedCrates.length > 0) {
        preloadVisibleCrateTrees(unloadedCrates);
      }
    }
  }, [docTree.value]);

  // Convert doc tree nodes → shared TreeView nodes
  const sharedNodes = useMemo(
    () => docTree.value.map(convertNode),
    [docTree.value]
  );

  // Determine selected ID
  const selectedId = useMemo(() => {
    // Check if a file is open in code viewer
    if (codeViewerFile.value) {
      return `file:${codeViewerFile.value}`;
    }
    return selectedFilename.value ?? undefined;
  }, [selectedFilename.value, codeViewerFile.value]);

  // Use controlled expanded from store
  const expanded = expandedNodes.value;

  const handleToggle = useCallback((id: string) => {
    toggleNodeExpanded(id);
  }, []);

  const handleSelect = useCallback(async (node: SharedTreeNode<TreeNode>) => {
    const docNode = node.data;
    if (!docNode) return;

    if (docNode.type === 'doc') {
      selectDoc(docNode.id);
    } else if (docNode.type === 'file' && docNode.sourceFile) {
      openSourceFile(docNode.sourceFile);
    } else if (docNode.type === 'root') {
      if (docNode.id === 'agents') {
        openCategoryPage('page:agent-docs');
      } else if (docNode.id === 'crates') {
        openCategoryPage('page:crate-docs');
      }
      toggleNodeExpanded(docNode.id);
    } else if (docNode.type === 'crate') {
      if (!docNode.children || docNode.children.length === 0) {
        await loadCrateModules(docNode.crateName!);
      }
      toggleNodeExpanded(docNode.id);
      openCrateDoc(docNode.crateName!);
    } else if (docNode.type === 'module') {
      openCrateDoc(docNode.crateName!, docNode.modulePath);
      const hasChildren = docNode.children && docNode.children.length > 0;
      if (hasChildren) {
        toggleNodeExpanded(docNode.id);
      }
    } else if (docNode.children && docNode.children.length > 0) {
      toggleNodeExpanded(docNode.id);
    }
  }, []);

  return (
    <div class="sidebar-content">
      <FileTree<TreeNode>
        nodes={sharedNodes}
        selectedId={selectedId}
        onSelect={handleSelect}
        expanded={expanded}
        onToggle={handleToggle}
        loading={isLoading.value && docTree.value.length === 0}
        emptyMessage="No documents found"
      />
    </div>
  );
}

// ── Node conversion ──

function convertNode(node: TreeNode): SharedTreeNode<TreeNode> {
  const hasChildren = node.children && node.children.length > 0;
  const canExpand = hasChildren || node.type === 'crate';

  const shared: SharedTreeNode<TreeNode> = {
    id: node.type === 'file' && node.sourceFile ? `file:${node.sourceFile.rel_path}` : node.id,
    label: node.label,
    icon: getNodeIcon(node),
    data: node,
    tooltip: node.data ? buildDocTooltip(node.data) : undefined,
  };

  if (hasChildren) {
    shared.children = node.children!.map(convertNode);
    shared.badge = node.children!.length;
  } else if (canExpand) {
    // Crate nodes that can expand but haven't loaded yet
    shared.children = [];
  }

  return shared;
}

function buildDocTooltip(doc: { title: string; date?: string; summary?: string; tags?: string[]; status?: string | null }): ComponentChildren {
  return h('div', { class: 'doc-tooltip' },
    h('div', { class: 'doc-tooltip-title' }, doc.title),
    doc.date && h('div', { class: 'doc-tooltip-date' }, doc.date),
    doc.summary && h('div', { class: 'doc-tooltip-summary' }, doc.summary),
    doc.tags && doc.tags.length > 0 && h('div', { class: 'doc-tooltip-tags' }, doc.tags.join(', ')),
    doc.status && h('div', { class: 'doc-tooltip-status' }, `Status: ${doc.status}`),
  );
}

function getNodeIcon(node: TreeNode): ComponentChildren {
  switch (node.type) {
    case 'root':
    case 'category':
      return h(FolderIcon, null);
    case 'crate':
      return h(CrateIcon, null);
    case 'module':
      return (node.children && node.children.length > 0) ? h(FolderIcon, null) : h(ModuleIcon, null);
    case 'file':
      return h(SourceFileIcon, null);
    case 'doc':
    default:
      return h(FileIcon, null);
  }
}

// ── Icons ──

function FolderIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor">
      <path d="M10 4H4a2 2 0 00-2 2v12a2 2 0 002 2h16a2 2 0 002-2V8a2 2 0 00-2-2h-8l-2-2z" />
    </svg>
  );
}

function CrateIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor">
      <path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z" />
      <polyline points="3.27 6.96 12 12.01 20.73 6.96" fill="none" stroke="var(--bg-primary)" stroke-width="1.5" />
      <line x1="12" y1="22.08" x2="12" y2="12" stroke="var(--bg-primary)" stroke-width="1.5" />
    </svg>
  );
}

function ModuleIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M14.5 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V7.5L14.5 2z" />
      <polyline points="14 2 14 8 20 8" />
      <path d="M8 13h8M8 17h8" />
    </svg>
  );
}

function FileIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M14.5 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V7.5L14.5 2z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" y1="13" x2="8" y2="13" />
      <line x1="16" y1="17" x2="8" y2="17" />
    </svg>
  );
}

function SourceFileIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M14.5 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V7.5L14.5 2z" />
      <polyline points="14 2 14 8 20 8" />
      <polyline points="9 13 7 15 9 17" />
      <polyline points="15 13 17 15 15 17" />
    </svg>
  );
}
