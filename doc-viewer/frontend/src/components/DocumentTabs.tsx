import { TabBar } from '@context-engine/viewer-api-frontend';
import { openTabs, activeTabId, closeTab, setActiveTab } from '../store';

export function DocumentTabs() {
  const tabs = openTabs.value.map(tab => ({
    id: tab.filename,
    label: tab.title,
    icon: <DocIcon />,
    closeable: true,
  }));
  const activeId = activeTabId.value;
  
  if (tabs.length === 0) {
    return null;
  }
  
  return (
    <TabBar
      tabs={tabs}
      activeTabId={activeId}
      onSelect={setActiveTab}
      onClose={closeTab}
      resizableBottom={false}
    />
  );
}

function DocIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M14.5 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V7.5L14.5 2z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" y1="13" x2="8" y2="13" />
      <line x1="16" y1="17" x2="8" y2="17" />
    </svg>
  );
}
