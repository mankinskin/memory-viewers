import { 
  Header as SharedHeader, 
  DocumentIcon, 
  FilterIcon, 
  CloseIcon, 
  RefreshIcon,
  HomeIcon 
} from '@context-engine/viewer-api-frontend';
import { 
  showFilterPanel, 
  hasActiveFilters, 
  clearFilters, 
  loadDocs,
  openCategoryPage 
} from '../store';

interface DocHeaderProps {
  onMenuToggle?: () => void;
  onThemeToggle?: () => void;
}

export function Header({ onMenuToggle, onThemeToggle }: DocHeaderProps) {
  const handleFilterToggle = () => {
    showFilterPanel.value = !showFilterPanel.value;
  };

  const handleRefresh = () => {
    loadDocs();
  };

  const handleHome = () => {
    openCategoryPage('page:home');
  };

  const rightContent = (
    <div class="header-actions">
      <button 
        class="btn"
        onClick={handleHome}
        title="Home"
      >
        <HomeIcon size={12} /> Home
      </button>
      
      <button 
        class={`btn ${showFilterPanel.value ? 'btn-active' : ''}`}
        onClick={handleFilterToggle}
        title="Advanced Filters"
      >
        <FilterIcon size={12} /> Filters
      </button>
      
      {hasActiveFilters.value && (
        <button class="btn" onClick={clearFilters} title="Clear all filters">
          <CloseIcon size={12} /> Clear
        </button>
      )}
      
      <button class="btn" onClick={handleRefresh} title="Refresh documentation">
        <RefreshIcon size={12} /> Refresh
      </button>

      {onThemeToggle && (
        <button class="btn" onClick={onThemeToggle} title="Theme settings" aria-label="Theme settings">
          <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="5" />
            <line x1="12" y1="1" x2="12" y2="3" />
            <line x1="12" y1="21" x2="12" y2="23" />
            <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
            <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
            <line x1="1" y1="12" x2="3" y2="12" />
            <line x1="21" y1="12" x2="23" y2="12" />
            <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
            <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
          </svg>
          {' '}Theme
        </button>
      )}
    </div>
  );

  return (
    <SharedHeader 
      title="Doc Viewer"
      icon={<DocumentIcon size={20} />}
      subtitle="context-engine documentation"
      rightContent={rightContent}
      onMenuToggle={onMenuToggle}
    />
  );
}
