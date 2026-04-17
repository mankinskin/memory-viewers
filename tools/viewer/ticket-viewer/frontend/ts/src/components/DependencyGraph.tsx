/**
 * DependencyGraph — GPU-accelerated 3D dependency graph for ticket-viewer.
 *
 * Uses HypergraphViewCore from viewer-api when WebGPU is available; falls back
 * to the SVG-based GraphView when WebGPU is not supported or the overlay is
 * not yet initialized.
 */
import { JSX } from 'preact';
import { useEffect, useState, useMemo, useCallback } from 'preact/hooks';
import type { ComponentChildren } from 'preact';
import {
    HypergraphViewCore,
    overlayGpu,
    type LayoutNode,
} from '@context-engine/viewer-api-frontend';
import { getSubgraph } from '../api';
import { authToken, openTicketId, selectedWorkspace } from '../store';
import type { SubgraphResponse } from '../types';
import { GraphView } from './GraphView';
import { TicketCard } from './TicketCard';

// ── Color palettes ──

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

    // Map SubgraphResponse to GraphSnapshot (index-based node/edge format).
    const snapshot = useMemo(() => {
        if (!data || data.nodes.length === 0) return null;
        const idToIndex = new Map(data.nodes.map((n, i) => [n.id, i]));
        const nodes = data.nodes.map((n, i) => ({
            index: i,
            label: n.title?.trim() || n.id.slice(0, 8),
            width: 1,
        }));
        const edges = data.edges
            .filter((e) => idToIndex.has(e.from) && idToIndex.has(e.to))
            .map((e, j) => ({
                from: idToIndex.get(e.from)!,
                to: idToIndex.get(e.to)!,
                pattern_idx: 0,
                sub_index: j,
            }));
        return { nodes, edges };
    }, [data]);

    // Custom node renderer: ticket cards instead of default atom badges.
    const renderNode = useCallback((node: LayoutNode): ComponentChildren => {
        const ticket = data?.nodes[node.index];
        if (!ticket) return <span class="ticket-card-fallback">{node.label}</span>;
        return <TicketCard ticket={ticket} isRoot={ticket.id === rootId} />;
    }, [data, rootId]);

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

                {rootId && !loading && !error && snapshot && (
                    <div
                        class="graph-view__canvas-wrap"
                        style={{ position: 'relative', width: '100%', height: '100%' }}
                    >
                        <HypergraphViewCore
                            snapshot={snapshot}
                            currentEvent={null}
                            searchPath={null}
                            autoLayout={false}
                            snapshotEdges={snapshot.edges}
                            stepKey=""
                            renderNode={renderNode}
                        />
                    </div>
                )}
            </div>
        </div>
    );
}
