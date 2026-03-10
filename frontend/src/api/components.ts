import { apiRequest } from './client';
import type {
  Component,
  CreateComponentRequest,
  UpdateComponentRequest,
  ListResponse,
} from './types';

/** List all components (flat list; frontend reconstructs tree from parent_id). */
export function listComponents(): Promise<ListResponse<Component>> {
  return apiRequest<ListResponse<Component>>('GET', '/api/components');
}

/** Create a component (admin only). */
export function createComponent(req: CreateComponentRequest): Promise<Component> {
  return apiRequest<Component>('POST', '/api/components', req);
}

/** Update a component (admin only). */
export function updateComponent(id: number, req: UpdateComponentRequest): Promise<Component> {
  return apiRequest<Component>('PATCH', `/api/components/${id}`, req);
}

/** Delete a component (admin only; fails if it has children or tickets). */
export function deleteComponent(id: number): Promise<void> {
  return apiRequest<void>('DELETE', `/api/components/${id}`);
}
