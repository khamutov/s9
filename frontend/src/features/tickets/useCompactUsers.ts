import { useQuery } from '@tanstack/react-query';
import { listCompactUsers } from '../../api/users';

/** Fetches all active users as compact objects for pickers (e.g. owner selection). */
export function useCompactUsers() {
  return useQuery({
    queryKey: ['users', 'compact'],
    queryFn: listCompactUsers,
    staleTime: 5 * 60 * 1000,
  });
}
