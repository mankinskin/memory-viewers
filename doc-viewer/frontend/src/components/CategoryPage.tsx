import { categories, docTree, openDoc, openCrateDoc, openCategoryPage } from '../store';
import { DocumentIcon, CrateIcon, FolderIcon, HomeIcon } from '@context-engine/viewer-api-frontend';
import type { TreeNode } from '../types';

interface CategoryPageProps {
  pageId: string;
}

export function CategoryPage({ pageId }: CategoryPageProps) {
  switch (pageId) {
    case 'page:home':
      return <HomePage />;
    case 'page:agent-docs':
      return <AgentDocsPage />;
    case 'page:crate-docs':
      return <CrateDocsPage />;
    default:
      return null;
  }
}

function HomePage() {
  const tree = docTree.value;
  const agentRoot = tree.find(n => n.id === 'agents');
  const crateRoot = tree.find(n => n.id === 'crates');

  return (
    <div class="category-page">
      <div class="category-header">
        <HomeIcon size={32} />
        <h1>Documentation</h1>
      </div>
      <p class="category-description">
        Browse agent documentation and crate API docs.
      </p>
      
      <div class="category-grid">
        {agentRoot && (
          <CategoryCard
            icon={<FolderIcon size={24} />}
            title="Agent Docs"
            description={`${countDocs(agentRoot)} documents across ${agentRoot.children?.length || 0} categories`}
            onClick={() => openCategoryPage('page:agent-docs')}
          />
        )}
        {crateRoot && (
          <CategoryCard
            icon={<CrateIcon size={24} />}
            title="Crate Docs"
            description={`${crateRoot.children?.length || 0} crates with API documentation`}
            onClick={() => openCategoryPage('page:crate-docs')}
          />
        )}
      </div>
    </div>
  );
}

function AgentDocsPage() {
  const cats = categories.value;

  return (
    <div class="category-page">
      <div class="category-header">
        <FolderIcon size={32} />
        <h1>Agent Documentation</h1>
      </div>
      <p class="category-description">
        Guides, plans, bug reports, and other documentation maintained by agents.
      </p>

      <div class="category-list">
        {cats.map(cat => (
          <div key={cat.category} class="category-section">
            <h2 class="category-section-title">
              <FolderIcon size={16} />
              {formatCategoryName(cat.category)}
              <span class="count">({cat.count})</span>
            </h2>
            <div class="doc-grid">
              {cat.docs.slice(0, 6).map(doc => (
                <DocCard
                  key={doc.filename}
                  title={doc.title}
                  summary={doc.summary}
                  date={doc.date}
                  tags={doc.tags ?? []}
                  onClick={() => openDoc(doc.filename, doc.title)}
                />
              ))}
            </div>
            {cat.docs.length > 6 && (
              <p class="more-docs">
                +{cat.docs.length - 6} more documents
              </p>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function CrateDocsPage() {
  const tree = docTree.value;
  const crateRoot = tree.find(n => n.id === 'crates');
  const crates = crateRoot?.children || [];

  return (
    <div class="category-page">
      <div class="category-header">
        <CrateIcon size={32} />
        <h1>Crate Documentation</h1>
      </div>
      <p class="category-description">
        API documentation automatically generated from crate source code.
      </p>

      <div class="crate-grid">
        {crates.map(crate => (
          <CrateCard
            key={crate.id}
            name={crate.label}
            hasReadme={crate.hasReadme}
            onClick={() => openCrateDoc(crate.crateName!)}
          />
        ))}
      </div>
    </div>
  );
}

// Helper components

interface CategoryCardProps {
  icon: preact.JSX.Element;
  title: string;
  description: string;
  onClick: () => void;
}

function CategoryCard({ icon, title, description, onClick }: CategoryCardProps) {
  return (
    <button type="button" class="category-card" onClick={onClick}>
      <div class="category-card-icon">{icon}</div>
      <div class="category-card-content">
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
    </button>
  );
}

interface DocCardProps {
  title: string;
  summary: string;
  date: string;
  tags: string[];
  onClick: () => void;
}

function DocCard({ title, summary, date, tags, onClick }: DocCardProps) {
  return (
    <button type="button" class="doc-card" onClick={onClick}>
      <div class="doc-card-header">
        <DocumentIcon size={14} />
        <span class="doc-card-date">{formatDate(date)}</span>
      </div>
      <h4 class="doc-card-title">{title}</h4>
      {summary && <p class="doc-card-summary">{summary}</p>}
      {tags?.length > 0 && (
        <div class="doc-card-tags">
          {tags.slice(0, 3).map(tag => (
            <span key={tag} class="doc-card-tag">#{tag}</span>
          ))}
        </div>
      )}
    </button>
  );
}

interface CrateCardProps {
  name: string;
  hasReadme?: boolean;
  onClick: () => void;
}

function CrateCard({ name, hasReadme, onClick }: CrateCardProps) {
  return (
    <button type="button" class="crate-card" onClick={onClick}>
      <CrateIcon size={20} />
      <span class="crate-card-name">{name}</span>
      {hasReadme && <span class="crate-card-badge">README</span>}
    </button>
  );
}

// Utility functions

function countDocs(node: TreeNode): number {
  if (node.type === 'doc') return 1;
  return node.children?.reduce((sum, child) => sum + countDocs(child), 0) || 0;
}

function formatCategoryName(name: string): string {
  return name
    .split('-')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

function formatDate(date: string): string {
  if (!date || date.length !== 8) return date;
  return `${date.slice(0, 4)}-${date.slice(4, 6)}-${date.slice(6, 8)}`;
}
