import type { AuthUser } from '../../api/types';

/** Shape of the auth context value. */
export interface AuthState {
  user: AuthUser | null;
  isLoading: boolean;
  login: (login: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
}
