import { useMutation, useQueryClient } from '@tanstack/react-query';
import { editComment, deleteComment } from '../../api/comments';
import type { EditCommentRequest } from '../../api/types';

/** Mutation hook for editing a comment. Invalidates comments cache on success. */
export function useEditComment(ticketId: number) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ commentNum, req }: { commentNum: number; req: EditCommentRequest }) =>
      editComment(ticketId, commentNum, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tickets', ticketId, 'comments'] });
    },
  });
}

/** Mutation hook for deleting a comment (admin only). Invalidates comments cache on success. */
export function useDeleteComment(ticketId: number) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (commentNum: number) => deleteComment(ticketId, commentNum),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tickets', ticketId, 'comments'] });
      queryClient.invalidateQueries({ queryKey: ['tickets', ticketId] });
    },
  });
}
