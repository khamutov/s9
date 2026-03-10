import { useQuery } from '@tanstack/react-query';
import { listUsers } from '../../api/users';

/** Fetches the list of active users. */
export function useUsers() {
  return useQuery({
    queryKey: ['users'],
    queryFn: () => listUsers(false),
  });
}
