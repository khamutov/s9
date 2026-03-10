import { useQuery } from '@tanstack/react-query';
import { listComponents } from '../../api/components';

/** Fetches the flat list of all components. */
export function useComponents() {
  return useQuery({
    queryKey: ['components'],
    queryFn: listComponents,
  });
}
