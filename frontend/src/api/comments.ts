import { apiRequest } from './client';
import type { Comment, CreateCommentRequest, EditCommentRequest, ListResponse } from './types';

/** List all comments for a ticket. */
export function listComments(
  ticketId: number,
  includeEdits = false,
): Promise<ListResponse<Comment>> {
  const qs = includeEdits ? '?include_edits=true' : '';
  return apiRequest<ListResponse<Comment>>('GET', `/api/tickets/${ticketId}/comments${qs}`);
}

/** Create a new comment on a ticket. */
export function createComment(ticketId: number, req: CreateCommentRequest): Promise<Comment> {
  return apiRequest<Comment>('POST', `/api/tickets/${ticketId}/comments`, req);
}

/** Edit an existing comment. */
export function editComment(
  ticketId: number,
  commentNum: number,
  req: EditCommentRequest,
): Promise<Comment> {
  return apiRequest<Comment>('PATCH', `/api/tickets/${ticketId}/comments/${commentNum}`, req);
}

/** Delete a comment (admin only, comment #0 protected). */
export function deleteComment(ticketId: number, commentNum: number): Promise<void> {
  return apiRequest<void>('DELETE', `/api/tickets/${ticketId}/comments/${commentNum}`);
}
