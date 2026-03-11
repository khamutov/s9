import { createContext } from 'react';
import type { AuthState } from './authState';

/** Auth context — consumed via useAuth hook, provided by AuthProvider. */
export const AuthContext = createContext<AuthState>({
  user: null,
  isLoading: true,
  login: () => Promise.resolve(),
  logout: () => Promise.resolve(),
  refreshUser: () => Promise.resolve(),
});
