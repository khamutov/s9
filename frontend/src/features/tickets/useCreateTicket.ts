import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createTicket } from '../../api/tickets';
import type { CreateTicketRequest } from '../../api/types';

/** Mutation hook for creating a new ticket. Invalidates ticket list on success. */
export function useCreateTicket() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateTicketRequest) => createTicket(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tickets'] });
    },
  });
}
