import './TicketCard.css';
import type { SubgraphNode } from '../types';

interface TicketCardProps {
    ticket: SubgraphNode;
    /** Whether this card is the root/focus of the current subgraph view. */
    isRoot?: boolean;
}

export function TicketCard({ ticket, isRoot }: TicketCardProps) {
    const shortId = ticket.id.slice(0, 8);
    const state = ticket.state ?? 'unknown';

    return (
        <div class={`ticket-card${isRoot ? ' ticket-card--root' : ''}`}>
            <div class="ticket-card-header">
                <span class={`ticket-state-badge ticket-state-badge--${state}`}>
                    {state}
                </span>
                <span class="ticket-short-id">#{shortId}</span>
            </div>
            <div class="ticket-card-title">
                {ticket.title ?? shortId}
            </div>
        </div>
    );
}
