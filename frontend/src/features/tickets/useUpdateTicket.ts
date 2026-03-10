import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updateTicket } from '../../api/tickets';
import type { Ticket, UpdateTicketRequest } from '../../api/types';

/** Mutation hook for PATCH /api/tickets/:id with optimistic cache update. */
export function useUpdateTicket(ticketId: number) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (req: UpdateTicketRequest) => updateTicket(ticketId, req),

    onMutate: async (req) => {
      await queryClient.cancelQueries({ queryKey: ['tickets', ticketId] });
      const previous = queryClient.getQueryData<Ticket>(['tickets', ticketId]);

      if (previous) {
        queryClient.setQueryData<Ticket>(['tickets', ticketId], {
          ...previous,
          ...req,
          // Preserve nested objects that the patch doesn't replace
          owner: previous.owner,
          component: previous.component,
          cc: previous.cc,
          milestones: previous.milestones,
          created_by: previous.created_by,
        });
      }

      return { previous };
    },

    onError: (_err, _req, context) => {
      if (context?.previous) {
        queryClient.setQueryData(['tickets', ticketId], context.previous);
      }
    },

    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['tickets', ticketId] });
      queryClient.invalidateQueries({ queryKey: ['tickets'], exact: false });
    },
  });
}
