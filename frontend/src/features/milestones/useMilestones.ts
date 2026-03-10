import { useQuery } from '@tanstack/react-query';
import { listMilestones } from '../../api/milestones';

/** Fetches the list of open milestones. */
export function useMilestones() {
  return useQuery({
    queryKey: ['milestones'],
    queryFn: () => listMilestones('open'),
  });
}
