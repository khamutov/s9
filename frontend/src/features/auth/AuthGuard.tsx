import { Navigate, Outlet } from 'react-router';
import { useAuth } from './useAuth';

/** Redirects to /login when the user is not authenticated. */
export default function AuthGuard() {
  const { user, isLoading } = useAuth();

  if (isLoading) return null;
  if (!user) return <Navigate to="/login" replace />;

  return <Outlet />;
}
