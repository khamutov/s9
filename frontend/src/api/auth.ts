import { apiRequest } from './client';
import type { AuthUser, LoginRequest } from './types';

/** Log in with username and password. Sets session cookie. */
export function login(req: LoginRequest): Promise<AuthUser> {
  return apiRequest<AuthUser>('POST', '/api/auth/login', req);
}

/** Destroy the current session. */
export function logout(): Promise<void> {
  return apiRequest<void>('POST', '/api/auth/logout');
}

/** Get the currently authenticated user, or throw 401. */
export function getMe(): Promise<AuthUser> {
  return apiRequest<AuthUser>('GET', '/api/auth/me');
}
