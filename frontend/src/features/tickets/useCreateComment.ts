import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createComment } from '../../api/comments';
import type { CreateCommentRequest } from '../../api/types';

/** Mutation hook for creating a comment on a ticket. Invalidates comments cache on success. */
export function useCreateComment(ticketId: number) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (req: CreateCommentRequest) => createComment(ticketId, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tickets', ticketId, 'comments'] });
      queryClient.invalidateQueries({ queryKey: ['tickets', ticketId] });
    },
  });
}
