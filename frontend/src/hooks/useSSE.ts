import { useEffect, useRef } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { SSEEventType } from '../api/types';

/**
 * Maps SSE event types to TanStack Query keys that should be invalidated.
 *
 * Static keys are always invalidated. Dynamic keys are built from event
 * payload when an entity ID is present.
 */
const EVENT_KEY_MAP: Record<SSEEventType, string[][]> = {
  ticket_created: [['tickets']],
  ticket_updated: [['tickets']],
  comment_created: [['tickets']],
  comment_updated: [['tickets']],
  comment_deleted: [['tickets']],
};

/** Parsed SSE event payload. */
interface SSEPayload {
  id?: number;
  ticket_id?: number;
  [key: string]: unknown;
}

/**
 * Connects to the SSE event stream and invalidates relevant TanStack Query
 * caches on each event. Uses native `EventSource` reconnection.
 *
 * @param enabled - Whether the SSE connection should be active (typically
 *   true when authenticated, false otherwise).
 */
export function useSSE(enabled: boolean): void {
  const queryClient = useQueryClient();
  const sourceRef = useRef<EventSource | null>(null);

  useEffect(() => {
    if (!enabled) {
      sourceRef.current?.close();
      sourceRef.current = null;
      return;
    }

    const es = new EventSource('/api/events');
    sourceRef.current = es;

    const eventTypes: SSEEventType[] = [
      'ticket_created',
      'ticket_updated',
      'comment_created',
      'comment_updated',
      'comment_deleted',
    ];

    function handleEvent(eventType: SSEEventType, event: MessageEvent) {
      // Invalidate static query keys for this event type.
      const keys = EVENT_KEY_MAP[eventType] ?? [];
      for (const key of keys) {
        queryClient.invalidateQueries({ queryKey: key });
      }

      // Invalidate specific detail queries when an entity ID is present.
      try {
        const data: SSEPayload = JSON.parse(event.data);
        const entityId = data.ticket_id ?? data.id;
        if (entityId != null) {
          queryClient.invalidateQueries({ queryKey: ['tickets', entityId] });
        }
      } catch {
        // Payload parse failure — static invalidation already happened.
      }
    }

    for (const type of eventTypes) {
      es.addEventListener(type, (e) => handleEvent(type, e as MessageEvent));
    }

    // Auth expiry: EventSource fires 'error' on non-200 responses.
    // Close and let the app handle redirect to login.
    es.addEventListener('error', () => {
      if (es.readyState === EventSource.CLOSED) {
        es.close();
        sourceRef.current = null;
      }
    });

    return () => {
      es.close();
      sourceRef.current = null;
    };
  }, [enabled, queryClient]);
}
