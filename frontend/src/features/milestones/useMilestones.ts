import { useQuery } from '@tanstack/react-query';
import { listMilestones } from '../../api/milestones';
import type { MilestoneStatus } from '../../api/types';

/** Fetches milestones with an optional status filter. */
export function useMilestones(status?: MilestoneStatus) {
  return useQuery({
    queryKey: ['milestones', status],
    queryFn: () => listMilestones(status),
  });
}
