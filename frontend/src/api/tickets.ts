import { apiRequest } from './client';
import type {
  Ticket,
  CreateTicketRequest,
  UpdateTicketRequest,
  SearchResult,
  MuteStatus,
} from './types';

/** Query parameters for listing/searching tickets. */
export interface TicketListParams {
  q?: string;
  cursor?: string;
  page?: number;
  page_size?: number;
  sort?: 'updated_at' | 'created_at' | 'priority' | 'status' | 'id';
  order?: 'asc' | 'desc';
}

function buildQuery(params: TicketListParams): string {
  const sp = new URLSearchParams();
  if (params.q) sp.set('q', params.q);
  if (params.cursor) sp.set('cursor', params.cursor);
  if (params.page != null) sp.set('page', String(params.page));
  if (params.page_size != null) sp.set('page_size', String(params.page_size));
  if (params.sort) sp.set('sort', params.sort);
  if (params.order) sp.set('order', params.order);
  const qs = sp.toString();
  return qs ? `?${qs}` : '';
}

/** List or search tickets with pagination. */
export function listTickets(params: TicketListParams = {}): Promise<SearchResult<Ticket>> {
  return apiRequest<SearchResult<Ticket>>('GET', `/api/tickets${buildQuery(params)}`);
}

/** Get a single ticket by ID. */
export function getTicket(id: number): Promise<Ticket> {
  return apiRequest<Ticket>('GET', `/api/tickets/${id}`);
}

/** Create a new ticket. */
export function createTicket(req: CreateTicketRequest): Promise<Ticket> {
  return apiRequest<Ticket>('POST', '/api/tickets', req);
}

/** Partially update a ticket. */
export function updateTicket(id: number, req: UpdateTicketRequest): Promise<Ticket> {
  return apiRequest<Ticket>('PATCH', `/api/tickets/${id}`, req);
}

/** Mute notifications for a ticket (idempotent). */
export function muteTicket(id: number): Promise<void> {
  return apiRequest<void>('POST', `/api/tickets/${id}/mute`);
}

/** Unmute notifications for a ticket (idempotent). */
export function unmuteTicket(id: number): Promise<void> {
  return apiRequest<void>('DELETE', `/api/tickets/${id}/mute`);
}

/** Get mute status for a ticket. */
export function getMuteStatus(id: number): Promise<MuteStatus> {
  return apiRequest<MuteStatus>('GET', `/api/tickets/${id}/mute`);
}
