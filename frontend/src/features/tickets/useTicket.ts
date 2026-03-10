import { useQuery } from '@tanstack/react-query';
import { getTicket } from '../../api/tickets';

/** Fetches a single ticket by ID with TanStack Query. */
export function useTicket(id: number) {
  return useQuery({
    queryKey: ['tickets', id],
    queryFn: () => getTicket(id),
    enabled: !isNaN(id),
  });
}
