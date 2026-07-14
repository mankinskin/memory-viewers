import { useEffect, useState, useCallback } from '@context-engine/viewer-api-frontend';
import { Sidebar as SharedSidebar, ThemeSettings, WgpuOverlay } from '@context-engine/viewer-api-frontend';
import { DOC_VIEWER_SCHEMA } from './gpu-schema';
import { Header } from './components/Header';
import { Sidebar } from './components/Sidebar';
import { Breadcrumbs } from './components/Breadcrumbs';
import { DocumentTabs } from './components/DocumentTabs';
import { DocViewer } from './components/DocViewer';
import { FilterPanel } from './components/FilterPanel';
import { FileViewer } from './components/FileViewer';
import { loadDocs, initUrlListener, codeViewerFile, closeCodeViewer, totalDocs, isLoading, docTree } from './store';
import { themeSettingsStore } from './theme';
import '@context-engine/viewer-api-frontend/styles/code-viewer.css';

export function App() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [showTheme, setShowTheme] = useState(false);
  // Ensure theme store module is initialised on first render.
  void themeSettingsStore;

  useEffect(() => {
    loadDocs();
    initUrlListener();
  }, []);

  const showCodeViewer = codeViewerFile.value !== null;

  const toggleMobileSidebar = useCallback(() => {
    setMobileOpen(prev => !prev);
  }, []);

  const closeMobileSidebar = useCallback(() => {
    setMobileOpen(false);
  }, []);

  const toggleTheme = useCallback(() => {
    setShowTheme(prev => !prev);
  }, []);

  return (
    <div class="app">
      <WgpuOverlay schema={DOC_VIEWER_SCHEMA} />
      <Header onMenuToggle={toggleMobileSidebar} onThemeToggle={toggleTheme} />
      {showTheme && (
        <div class="theme-overlay" onClick={(e) => { if (e.target === e.currentTarget) setShowTheme(false); }}>
          <div class="theme-overlay__panel">
            <ThemeSettings store={themeSettingsStore} />
          </div>
        </div>
      )}
      <FilterPanel />
      <div class="main-layout">
        <SharedSidebar
          title="Documentation"
          badge={totalDocs.value}
          collapsible
          resizable
          initialWidth={280}
          loading={isLoading.value && docTree.value.length === 0}
          isEmpty={!isLoading.value && docTree.value.length === 0}
          emptyMessage="No documents found"
          mobileOpen={mobileOpen}
          onMobileClose={closeMobileSidebar}
        >
          <Sidebar />
        </SharedSidebar>
        <main class="content">
          <Breadcrumbs />
          <DocumentTabs />
          <div class="content-panels">
            {showCodeViewer ? (
              <div class="code-panel full-width">
                <div class="code-panel-header">
                  <span class="code-panel-title">{codeViewerFile.value}</span>
                  <button class="code-panel-close" onClick={closeCodeViewer} title="Close">×</button>
                </div>
                <FileViewer />
              </div>
            ) : (
              <DocViewer />
            )}
          </div>
        </main>
      </div>
    </div>
  );
}
