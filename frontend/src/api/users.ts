import { apiRequest } from './client';
import type {
  CompactUser,
  User,
  CreateUserRequest,
  UpdateUserRequest,
  SetPasswordRequest,
  ListResponse,
} from './types';

/** List all active users as compact objects (any authenticated user). */
export function listCompactUsers(): Promise<ListResponse<CompactUser>> {
  return apiRequest<ListResponse<CompactUser>>('GET', '/api/users/compact');
}

/** List users (admin only). */
export function listUsers(includeInactive = false): Promise<ListResponse<User>> {
  const qs = includeInactive ? '?include_inactive=true' : '';
  return apiRequest<ListResponse<User>>('GET', `/api/users${qs}`);
}

/** Create a user (admin only). */
export function createUser(req: CreateUserRequest): Promise<User> {
  return apiRequest<User>('POST', '/api/users', req);
}

/** Update a user (self: display_name/email; admin: all fields). */
export function updateUser(id: number, req: UpdateUserRequest): Promise<User> {
  return apiRequest<User>('PATCH', `/api/users/${id}`, req);
}

/** Set a user's password. Self requires current_password; admin can set directly. */
export function setPassword(id: number, req: SetPasswordRequest): Promise<void> {
  return apiRequest<void>('POST', `/api/users/${id}/password`, req);
}
