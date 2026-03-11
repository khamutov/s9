import type { AuthUser } from '../../api/types';

/** Shape of the auth context value. */
export interface AuthState {
  user: AuthUser | null;
  isLoading: boolean;
  login: (login: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  /** Re-fetch the current user from the server (e.g. after profile update). */
  refreshUser: () => Promise<void>;
}
