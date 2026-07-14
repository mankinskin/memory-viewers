import { signal } from '@context-engine/viewer-api-frontend';
import { 
  showFilterPanel, 
  docTypeFilter, 
  tagFilter, 
  dateFromFilter, 
  dateToFilter,
  jqFilter,
  jqResults,
  isFilterLoading,
  allTags,
  allDocTypes,
  executeJqQuery,
  clearFilters,
  hasActiveFilters,
  openDoc
} from '../store';
import { SearchIcon, FilterIcon, CloseIcon } from '@context-engine/viewer-api-frontend';

// Local state for custom JQ input
const customJq = signal('');

// Common JQ presets for documentation
const JQ_PRESETS = [
  { label: 'All guides', jq: 'select(.doc_type == "guides")' },
  { label: 'All plans', jq: 'select(.doc_type == "plans")' },
  { label: 'Recent (30 days)', jq: 'select(.date >= "20260121")' }, // Adjust dynamically
  { label: 'With tag #testing', jq: 'select(.tags | any(. == "testing"))' },
  { label: 'Title contains "search"', jq: 'select(.title | test("search"; "i"))' },
  { label: 'Bug reports', jq: 'select(.doc_type == "bug-reports")' },
];

export function FilterPanel() {
  if (!showFilterPanel.value) return null;

  const handleDocTypeChange = (e: Event) => {
    const value = (e.target as HTMLSelectElement).value;
    docTypeFilter.value = value;
    executeJqQuery();
  };

  const handleTagChange = (e: Event) => {
    const value = (e.target as HTMLSelectElement).value;
    tagFilter.value = value;
    executeJqQuery();
  };

  const handleDateFromChange = (e: Event) => {
    const value = (e.target as HTMLInputElement).value;
    // Convert YYYY-MM-DD to YYYYMMDD
    dateFromFilter.value = value.replace(/-/g, '');
    executeJqQuery();
  };

  const handleDateToChange = (e: Event) => {
    const value = (e.target as HTMLInputElement).value;
    dateToFilter.value = value.replace(/-/g, '');
    executeJqQuery();
  };

  const handleCustomJq = (e: Event) => {
    e.preventDefault();
    if (customJq.value.trim()) {
      executeJqQuery(customJq.value);
    }
  };

  const handlePreset = (jq: string) => {
    customJq.value = jq;
    executeJqQuery(jq);
  };

  const handleResultClick = (filename: string, title: string) => {
    openDoc(filename, title);
  };

  const handleClear = () => {
    customJq.value = '';
    clearFilters();
  };

  // Format date from YYYYMMDD to YYYY-MM-DD for input
  const formatDateForInput = (date: string) => {
    if (date.length !== 8) return '';
    return `${date.slice(0, 4)}-${date.slice(4, 6)}-${date.slice(6, 8)}`;
  };

  return (
    <div class="filter-panel">
      <div class="filter-panel-header">
        <h3><FilterIcon size={14} /> Filters</h3>
        {hasActiveFilters.value && (
          <button class="btn btn-sm" onClick={handleClear}>
            <CloseIcon size={10} /> Clear
          </button>
        )}
      </div>
      
      <div class="filter-columns">
        {/* Basic Filters Column */}
        <div class="filter-column">
          <h4>Basic Filters</h4>
          
          <div class="filter-section">
            <label>Document Type</label>
            <select 
              class="filter-select" 
              value={docTypeFilter.value}
              onChange={handleDocTypeChange}
            >
              <option value="">All types</option>
              {allDocTypes.value.map(type => (
                <option key={type} value={type}>{type}</option>
              ))}
            </select>
          </div>
          
          <div class="filter-section">
            <label>Tag</label>
            <select 
              class="filter-select" 
              value={tagFilter.value}
              onChange={handleTagChange}
            >
              <option value="">All tags</option>
              {allTags.value.map(tag => (
                <option key={tag} value={tag}>{tag}</option>
              ))}
            </select>
          </div>
          
          <div class="filter-section">
            <label>Date Range</label>
            <div class="date-range">
              <input 
                type="date" 
                class="filter-input"
                value={formatDateForInput(dateFromFilter.value)}
                onChange={handleDateFromChange}
                placeholder="From"
              />
              <span>to</span>
              <input 
                type="date" 
                class="filter-input"
                value={formatDateForInput(dateToFilter.value)}
                onChange={handleDateToChange}
                placeholder="To"
              />
            </div>
          </div>
        </div>
        
        {/* JQ Query Column */}
        <div class="filter-column filter-column-wide">
          <h4>JQ Query</h4>
          
          <div class="filter-section">
            <label>Quick Presets</label>
            <div class="filter-presets">
              {JQ_PRESETS.map(preset => (
                <button 
                  key={preset.jq}
                  class="btn btn-sm btn-preset"
                  onClick={() => handlePreset(preset.jq)}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>
          
          <div class="filter-section">
            <label>Custom JQ Expression</label>
            <form class="jq-form" onSubmit={handleCustomJq}>
              <input 
                type="text" 
                class="jq-input"
                placeholder='e.g., select(.title | test("pattern"; "i"))'
                value={customJq.value}
                onInput={(e) => customJq.value = (e.target as HTMLInputElement).value}
              />
              <button type="submit" class="btn btn-primary" disabled={isFilterLoading.value}>
                <SearchIcon size={12} /> Query
              </button>
            </form>
            {jqFilter.value && (
              <div class="active-filter">
                <code>{jqFilter.value}</code>
              </div>
            )}
          </div>
        </div>
      </div>
      
      {/* Results Section */}
      {jqResults.value !== null && (
        <div class="filter-results">
          <h4>Results ({jqResults.value.length})</h4>
          {jqResults.value.length === 0 ? (
            <p class="no-results">No documents match the query</p>
          ) : (
            <div class="results-list">
              {jqResults.value.slice(0, 50).map((result) => (
                <div 
                  key={result.filename}
                  class="result-item"
                  onClick={() => handleResultClick(result.filename, result.title)}
                >
                  <span class="result-type">{result.doc_type}</span>
                  <span class="result-title">{result.title}</span>
                  <span class="result-date">{result.date}</span>
                </div>
              ))}
              {jqResults.value.length > 50 && (
                <p class="more-results">...and {jqResults.value.length - 50} more</p>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
