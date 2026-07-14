import { activeDoc, activeTabId, openTabs, openCrateDoc, openCategoryPage, codeViewerFile, closeCodeViewer } from '../store';
import { ChevronRightIcon, DocumentIcon, CrateIcon, FolderIcon, HomeIcon } from '@context-engine/viewer-api-frontend';

// Source file icon for breadcrumbs
function SourceFileIcon({ size = 12 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="currentColor">
      <path d="M14 4.5V14a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V2a2 2 0 0 1 2-2h5.5L14 4.5zm-3 0A1.5 1.5 0 0 1 9.5 3V1H4a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V4.5h-2z"/>
      <path d="M4.5 12.5A.5.5 0 0 1 5 12h6a.5.5 0 0 1 0 1H5a.5.5 0 0 1-.5-.5zm0-2A.5.5 0 0 1 5 10h6a.5.5 0 0 1 0 1H5a.5.5 0 0 1-.5-.5zm0-2A.5.5 0 0 1 5 8h6a.5.5 0 0 1 0 1H5a.5.5 0 0 1-.5-.5zm0-2A.5.5 0 0 1 5 6h6a.5.5 0 0 1 0 1H5a.5.5 0 0 1-.5-.5z"/>
    </svg>
  );
}

interface BreadcrumbPart {
  label: string;
  type: 'home' | 'root' | 'category' | 'crate' | 'module' | 'doc' | 'file';
  onClick?: () => void;
}

function parseBreadcrumbs(filename: string | null, title: string, docType: string): BreadcrumbPart[] {
  if (!filename) return [];
  
  const parts: BreadcrumbPart[] = [];
  
  // Handle category pages themselves
  if (filename === 'page:home') {
    parts.push({ label: 'Home', type: 'home' });
    return parts;
  }
  
  if (filename === 'page:agent-docs') {
    parts.push({
      label: 'Home',
      type: 'home',
      onClick: () => openCategoryPage('page:home'),
    });
    parts.push({ label: 'Agent Docs', type: 'root' });
    return parts;
  }
  
  if (filename === 'page:crate-docs') {
    parts.push({
      label: 'Home',
      type: 'home',
      onClick: () => openCategoryPage('page:home'),
    });
    parts.push({ label: 'Crate Docs', type: 'root' });
    return parts;
  }
  
  // Always start with Home
  parts.push({
    label: 'Home',
    type: 'home',
    onClick: () => openCategoryPage('page:home'),
  });
  
  if (filename.startsWith('crate:')) {
    // Crate documentation: crate:crateName or crate:crateName:module/path
    parts.push({
      label: 'Crate Docs',
      type: 'root',
      onClick: () => openCategoryPage('page:crate-docs'),
    });
    
    const colonParts = filename.split(':');
    const crateName = colonParts[1];
    const modulePath = colonParts.slice(2).join(':');
    
    // Crate name - clicking navigates to crate root (index.yaml)
    parts.push({
      label: crateName,
      type: 'crate',
      onClick: modulePath ? () => openCrateDoc(crateName) : undefined,
    });
    
    if (modulePath) {
      // Split module path by / and add each as a breadcrumb
      const modules = modulePath.split('/');
      modules.forEach((mod, idx) => {
        const isLast = idx === modules.length - 1;
        // Build partial path up to this module
        const partialPath = modules.slice(0, idx + 1).join('/');
        
        parts.push({
          label: mod,
          type: isLast ? 'doc' : 'module',
          // Only add onClick for non-last items (intermediate modules)
          onClick: isLast ? undefined : () => openCrateDoc(crateName, partialPath),
        });
      });
    }
  } else {
    // Agent documentation
    parts.push({
      label: 'Agent Docs',
      type: 'root',
      onClick: () => openCategoryPage('page:agent-docs'),
    });
    
    // Add category from doc_type
    if (docType && docType !== 'crate') {
      const categoryLabel = formatCategoryName(docType);
      parts.push({ label: categoryLabel, type: 'category' });
    }
    
    // Add document title (current, not clickable)
    parts.push({ label: title, type: 'doc' });
  }
  
  return parts;
}

function formatCategoryName(name: string): string {
  return name
    .split('-')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

function getIcon(type: BreadcrumbPart['type']) {
  switch (type) {
    case 'home':
      return <HomeIcon size={12} />;
    case 'root':
      return <FolderIcon size={12} />;
    case 'crate':
      return <CrateIcon size={12} />;
    case 'category':
    case 'module':
      return <FolderIcon size={12} />;
    case 'file':
      return <SourceFileIcon size={12} />;
    case 'doc':
      return <DocumentIcon size={12} />;
    default:
      return null;
  }
}

// Parse breadcrumbs for a source file path
function parseSourceFileBreadcrumbs(filePath: string): BreadcrumbPart[] {
  const parts: BreadcrumbPart[] = [];
  
  // Always start with Home
  parts.push({
    label: 'Home',
    type: 'home',
    onClick: () => {
      closeCodeViewer();
      openCategoryPage('page:home');
    },
  });
  
  // Add Crate Docs root
  parts.push({
    label: 'Crate Docs',
    type: 'root',
    onClick: () => {
      closeCodeViewer();
      openCategoryPage('page:crate-docs');
    },
  });
  
  // Parse the file path to extract crate and module info
  // Paths look like: src/some/module/file.rs or agents/docs/module/index.yaml
  const pathParts = filePath.split('/');
  
  // Find crate name by looking for common patterns
  // The rel_path typically starts after the crate folder
  // e.g., "src/foo/bar.rs" - we need to figure out the crate
  // For now, just show the path segments
  
  // Show just the filename as the current item
  const filename = pathParts[pathParts.length - 1];
  
  // Add intermediate path segments as module breadcrumbs
  if (pathParts.length > 1) {
    for (let i = 0; i < pathParts.length - 1; i++) {
      parts.push({
        label: pathParts[i],
        type: 'module',
        onClick: () => closeCodeViewer(), // Just close viewer for path segments
      });
    }
  }
  
  // Add the file itself (not clickable - it's current)
  parts.push({
    label: filename,
    type: 'file',
  });
  
  return parts;
}

export function Breadcrumbs() {
  const sourceFile = codeViewerFile.value;
  
  // If viewing a source file, show source file breadcrumbs
  if (sourceFile) {
    const parts = parseSourceFileBreadcrumbs(sourceFile);
    
    return (
      <div class="breadcrumbs">
        <nav class="breadcrumb-nav" aria-label="Breadcrumb">
          {parts.map((part, idx) => (
            <span key={idx} class="breadcrumb-item">
              {idx > 0 && (
                <span class="breadcrumb-separator">
                  <ChevronRightIcon size={12} />
                </span>
              )}
              {part.onClick ? (
                <button
                  type="button"
                  class="breadcrumb-label clickable"
                  title={part.label}
                  onClick={part.onClick}
                >
                  <span class="breadcrumb-icon">{getIcon(part.type)}</span>
                  {part.label}
                </button>
              ) : (
                <span 
                  class={`breadcrumb-label current`}
                  title={part.label}
                >
                  <span class="breadcrumb-icon">{getIcon(part.type)}</span>
                  {part.label}
                </span>
              )}
            </span>
          ))}
        </nav>
      </div>
    );
  }
  
  const doc = activeDoc.value;
  const activeId = activeTabId.value;
  const activeTab = openTabs.value.find(t => t.filename === activeId);
  
  if (!activeId || !activeTab) {
    return (
      <div class="breadcrumbs">
        <span class="breadcrumb-empty">Select a document to view</span>
      </div>
    );
  }
  
  const parts = parseBreadcrumbs(
    activeId,
    activeTab.title,
    doc?.doc_type ?? ''
  );
  
  return (
    <div class="breadcrumbs">
      <nav class="breadcrumb-nav" aria-label="Breadcrumb">
        {parts.map((part, idx) => (
          <span key={idx} class="breadcrumb-item">
            {idx > 0 && (
              <span class="breadcrumb-separator">
                <ChevronRightIcon size={12} />
              </span>
            )}
            {part.onClick ? (
              <button
                type="button"
                class="breadcrumb-label clickable"
                title={part.label}
                onClick={part.onClick}
              >
                <span class="breadcrumb-icon">{getIcon(part.type)}</span>
                {part.label}
              </button>
            ) : (
              <span 
                class={`breadcrumb-label current`}
                title={part.label}
              >
                <span class="breadcrumb-icon">{getIcon(part.type)}</span>
                {part.label}
              </span>
            )}
          </span>
        ))}
      </nav>
      {activeTab.isLoading && (
        <span class="breadcrumb-loading">Loading...</span>
      )}
    </div>
  );
}
