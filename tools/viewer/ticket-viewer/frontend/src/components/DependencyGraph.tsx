/**
 * DependencyGraph — GPU-accelerated 3D dependency graph for ticket-viewer.
 *
 * Uses Graph3DView from viewer-api when WebGPU is available; falls back to the
 * SVG-based GraphView when WebGPU is not supported or the overlay is not yet
 * initialized.
 */
import { JSX } from 'preact';
import { useEffect, useState, useCallback, useMemo } from 'preact/hooks';
import {
    Graph3DView,
    overlayGpu,
    type Graph3DNode,
    type Graph3DEdge,
} from '@context-engine/viewer-api-frontend';
import { getSubgraph } from '../api';
import { authToken, openTicketId, selectedWorkspace } from '../store';
import type { SubgraphResponse } from '../types';
import { GraphView } from './GraphView';

// ── Color palettes ──

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

const EDGE_COLORS: Record<string, string> = {
    depends_on: '#4a9eff',
    blocks: '#fc8181',
    linked: '#a0aec0',
};

// ── Helpers ──

function hasWebGPU(): boolean {
    return typeof navigator !== 'undefined' && 'gpu' in navigator;
}

// ── Components ──

export function DependencyGraph(): JSX.Element {
    const workspace = selectedWorkspace.value;
    const rootId = openTicketId.value;
    const token = authToken.value || undefined;

    const [data, setData] = useState<SubgraphResponse | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Use 3D rendering when WebGPU is supported and the overlay is initialized.
    // `overlayGpu` is a Preact signal — reading .value here makes this component
    // reactive to its changes (e.g. WgpuOverlay finishes GPU init).
    const gpu = overlayGpu.value;
    const canUse3D = hasWebGPU() && gpu !== null;

    useEffect(() => {
        if (!canUse3D || !workspace || !rootId) {
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
                if (!cancelled) setData(resp);
            })
            .catch((err: unknown) => {
                if (!cancelled) {
                    setData(null);
                    setError(String(err));
                }
            })
            .finally(() => {
                if (!cancelled) setLoading(false);
            });

        return () => {
            cancelled = true;
        };
    }, [canUse3D, workspace, rootId, token]);

    const g3dNodes = useMemo<Graph3DNode[]>(() => {
        if (!data) return [];
        return data.nodes.map((n) => ({
            id: n.id,
            label: n.title?.trim() || n.id.slice(0, 8),
            color: STATE_COLORS[n.state ?? ''] ?? '#6e7aa2',
            // Root ticket is displayed larger (depth 0 = root)
            size: n.depth === 0 ? 3 : 1,
        }));
    }, [data]);

    const g3dEdges = useMemo<Graph3DEdge[]>(() => {
        if (!data) return [];
        return data.edges.map((e) => ({
            source: e.from,
            target: e.to,
            color: EDGE_COLORS[e.kind],
        }));
    }, [data]);

    const handleNodeClick = useCallback((node: Graph3DNode) => {
        openTicketId.value = node.id;
    }, []);

    // ── Fallback: no WebGPU or overlay not yet ready ──
    if (!canUse3D) {
        return <GraphView />;
    }

    // ── 3D GPU rendering path ──
    return (
        <div class="graph-view">
            <div class="graph-view__header">Dependency Graph</div>
            <div class="graph-view__body">
                {!rootId && (
                    <p class="graph-view__note">
                        Select a ticket to explore its dependency graph.
                    </p>
                )}

                {rootId && loading && (
                    <p class="graph-view__note">Loading graph…</p>
                )}

                {rootId && !loading && error && (
                    <p class="graph-view__error" role="alert">
                        Failed to load graph: {error}
                    </p>
                )}

                {rootId && !loading && !error && data && data.nodes.length === 0 && (
                    <p class="graph-view__note">
                        No graph nodes were returned for this ticket.
                    </p>
                )}

                {rootId && !loading && !error && data && data.nodes.length > 0 && (
                    <div
                        class="graph-view__canvas-wrap"
                        style={{ position: 'relative', width: '100%', height: '100%' }}
                    >
                        <Graph3DView
                            nodes={g3dNodes}
                            edges={g3dEdges}
                            onNodeClick={handleNodeClick}
                            selectedNodeId={rootId}
                            layoutMode="hierarchical"
                        />
                    </div>
                )}
            </div>
        </div>
    );
}
