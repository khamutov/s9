import { useQuery } from '@tanstack/react-query';
import { listTickets, type TicketListParams } from '../../api/tickets';

/** Fetches paginated ticket list with TanStack Query. */
export function useTickets(params: TicketListParams = {}) {
  return useQuery({
    queryKey: ['tickets', params],
    queryFn: () => listTickets(params),
  });
}
