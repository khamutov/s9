import { useContext } from 'react';
import { AuthContext } from './AuthContext';
import type { AuthState } from './authState';

/** Access the current auth state from any component. */
export function useAuth(): AuthState {
  return useContext(AuthContext);
}
