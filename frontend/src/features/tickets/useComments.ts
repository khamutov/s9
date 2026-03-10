import { useQuery } from '@tanstack/react-query';
import { listComments } from '../../api/comments';

/** Fetches all comments for a ticket with TanStack Query. */
export function useComments(ticketId: number) {
  return useQuery({
    queryKey: ['tickets', ticketId, 'comments'],
    queryFn: () => listComments(ticketId),
    enabled: !isNaN(ticketId),
  });
}
