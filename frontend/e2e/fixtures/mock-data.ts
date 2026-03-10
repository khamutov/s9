import type { AuthUser } from '../../src/api/types';

/** Standard test user for authenticated E2E flows. */
export const TEST_USER: AuthUser = {
  id: 1,
  login: 'testuser',
  display_name: 'Test User',
  email: 'test@s9.dev',
  role: 'user',
};

/** Admin test user for E2E flows requiring elevated privileges. */
export const TEST_ADMIN: AuthUser = {
  id: 2,
  login: 'admin',
  display_name: 'Admin',
  email: 'admin@s9.dev',
  role: 'admin',
};
