import { JSX } from 'preact';
import { useEffect, useMemo, useState } from 'preact/hooks';
import { getSubgraph } from '../api';
import { authToken, openTicketId, selectedWorkspace } from '../store';
import type { SubgraphNode, SubgraphResponse } from '../types';

const STATE_COLORS: Record<string, string> = {
  open: '#4a9eff',
  'in-progress': '#f0a500',
  review: '#9b7fe8',
  validating: '#63b3ed',
  validated: '#48bb78',
  done: '#68d391',
  blocked: '#fc8181',
  cancelled: '#a0aec0',
};

interface NodePosition {
  x: number;
  y: number;
}

function getNodeLabel(node: SubgraphNode): string {
  return node.title?.trim() || node.id.slice(0, 8);
}

export function GraphView(): JSX.Element {
  const workspace = selectedWorkspace.value;
  const rootId = openTicketId.value;
  const token = authToken.value || undefined;

  const [data, setData] = useState<SubgraphResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!workspace || !rootId) {
      setData(null);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    void getSubgraph(workspace, rootId, { direction: 'both', depth: 2 }, token)
      .then((resp) => {
        if (!cancelled) {
          setData(resp);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setData(null);
          setError(String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [workspace, rootId, token]);

  const layout = useMemo(() => {
    if (!data || data.nodes.length === 0) {
      return {
        nodeMap: new Map<string, SubgraphNode>(),
        positions: new Map<string, NodePosition>(),
        width: 0,
        height: 0,
      };
    }

    const nodeMap = new Map<string, SubgraphNode>();
    const byDepth = new Map<number, SubgraphNode[]>();
    for (const node of data.nodes) {
      nodeMap.set(node.id, node);
      const list = byDepth.get(node.depth) ?? [];
      list.push(node);
      byDepth.set(node.depth, list);
    }

    const depthLevels = [...byDepth.keys()].sort((a, b) => a - b);
    const maxDepth = depthLevels.length > 0 ? depthLevels[depthLevels.length - 1] : 0;
    const maxRows = depthLevels.reduce((m, d) => Math.max(m, (byDepth.get(d) ?? []).length), 1);

    const horizontalStep = 190;
    const verticalStep = 78;
    const marginX = 60;
    const marginY = 48;

    const width = marginX * 2 + (maxDepth + 1) * horizontalStep;
    const height = marginY * 2 + Math.max(1, maxRows - 1) * verticalStep;

    const positions = new Map<string, NodePosition>();
    for (const depth of depthLevels) {
      const group = [...(byDepth.get(depth) ?? [])].sort((a, b) =>
        getNodeLabel(a).localeCompare(getNodeLabel(b)),
      );
      const rows = group.length;
      const x = marginX + depth * horizontalStep;
      const startY = (height - Math.max(1, rows - 1) * verticalStep) / 2;

      for (let index = 0; index < group.length; index += 1) {
        positions.set(group[index].id, {
          x,
          y: startY + index * verticalStep,
        });
      }
    }

    return { nodeMap, positions, width, height };
  }, [data]);

  const edges = useMemo(() => {
    if (!data) return [];
    return data.edges.filter(
      (edge) => layout.positions.has(edge.from) && layout.positions.has(edge.to),
    );
  }, [data, layout.positions]);

  return (
    <div class="graph-view">
      <div class="graph-view__header">Dependency Graph</div>
      <div class="graph-view__body">
        {!rootId && (
          <p class="graph-view__note">Select a ticket to explore its dependency graph.</p>
        )}

        {rootId && loading && <p class="graph-view__note">Loading graph...</p>}

        {rootId && !loading && error && (
          <p class="graph-view__error" role="alert">
            Failed to load graph: {error}
          </p>
        )}

        {rootId && !loading && !error && data && data.nodes.length === 0 && (
          <p class="graph-view__note">No graph nodes were returned for this ticket.</p>
        )}

        {rootId && !loading && !error && data && data.nodes.length > 0 && (
          <div class="graph-view__canvas-wrap">
            <svg
              class="graph-view__svg"
              viewBox={`0 0 ${layout.width} ${layout.height}`}
              aria-label="Ticket dependency graph"
            >
              <defs>
                <marker
                  id="graph-arrow"
                  markerWidth="10"
                  markerHeight="8"
                  refX="9"
                  refY="4"
                  orient="auto"
                >
                  <path d="M 0 0 L 10 4 L 0 8 z" class="graph-view__arrow" />
                </marker>
              </defs>

              {edges.map((edge) => {
                const fromPos = layout.positions.get(edge.from);
                const toPos = layout.positions.get(edge.to);
                if (!fromPos || !toPos) return null;

                return (
                  <line
                    key={`${edge.from}-${edge.to}-${edge.kind}`}
                    x1={fromPos.x}
                    y1={fromPos.y}
                    x2={toPos.x}
                    y2={toPos.y}
                    class="graph-view__edge"
                    markerEnd="url(#graph-arrow)"
                  />
                );
              })}

              {data.nodes.map((node) => {
                const pos = layout.positions.get(node.id);
                if (!pos) return null;
                const fill = STATE_COLORS[node.state ?? ''] ?? '#6e7aa2';
                const isRoot = node.id === rootId;

                return (
                  <g key={node.id} class="graph-view__node" transform={`translate(${pos.x}, ${pos.y})`}>
                    <circle
                      class="graph-view__node-dot"
                      r={isRoot ? 14 : 11}
                      fill={fill}
                      stroke={isRoot ? '#e5e9ff' : '#1a1b26'}
                      stroke-width={isRoot ? 2.5 : 1.5}
                    />
                    <text class="graph-view__node-label" x="18" y="4">
                      {getNodeLabel(node)}
                    </text>
                    <text class="graph-view__node-meta" x="18" y="19">
                      depth {node.depth}
                      {node.state ? ` • ${node.state}` : ''}
                    </text>
                  </g>
                );
              })}
            </svg>
          </div>
        )}
      </div>
    </div>
  );
}
