import { useMemo } from '@context-engine/viewer-api-frontend';
import { CodeViewer } from '@context-engine/viewer-api-frontend';
import { codeViewerFile, codeViewerContent, codeViewerLine } from '../store';
import { Marked } from 'marked';
import { markedHighlight } from 'marked-highlight';
import hljs from 'highlight.js';

// Configure marked with syntax highlighting (same as DocViewer)
const marked = new Marked(
  markedHighlight({
    langPrefix: 'hljs language-',
    highlight(code, lang) {
      if (lang && hljs.getLanguage(lang)) {
        try {
          return hljs.highlight(code, { language: lang }).value;
        } catch {
          // Fall through to auto-detection
        }
      }
      try {
        return hljs.highlightAuto(code).value;
      } catch {
        return code;
      }
    },
  }),
  {
    gfm: true,
    breaks: false,
  }
);

function isMarkdownFile(filename: string | null): boolean {
  if (!filename) return false;
  return filename.endsWith('.md') || filename.endsWith('.markdown');
}

export function FileViewer() {
  const filename = codeViewerFile.value;
  const content = codeViewerContent.value;
  
  const htmlContent = useMemo(() => {
    if (!isMarkdownFile(filename) || !content) return '';
    return marked.parse(content) as string;
  }, [filename, content]);
  
  // Render markdown files as HTML
  if (isMarkdownFile(filename)) {
    const displayName = filename?.split('/').pop() || filename || '';
    
    return (
      <div class="file-viewer markdown-viewer">
        <div class="markdown-header">
          <span class="markdown-filename" title={filename ?? undefined}>{displayName}</span>
        </div>
        <div class="markdown-content">
          <div 
            class="markdown-body" 
            dangerouslySetInnerHTML={{ __html: htmlContent }}
          />
        </div>
      </div>
    );
  }
  
  // Render code files with syntax highlighting
  return (
    <CodeViewer 
      file={codeViewerFile}
      content={codeViewerContent}
      highlightLine={codeViewerLine}
      placeholderMessage="Select a source file"
      placeholderIcon="📄"
    />
  );
}
