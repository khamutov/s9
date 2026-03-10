import { apiRequest } from './client';
import type {
  Milestone,
  CreateMilestoneRequest,
  UpdateMilestoneRequest,
  MilestoneStatus,
  ListResponse,
} from './types';

/** List milestones with optional status filter. */
export function listMilestones(status?: MilestoneStatus): Promise<ListResponse<Milestone>> {
  const qs = status ? `?status=${status}` : '';
  return apiRequest<ListResponse<Milestone>>('GET', `/api/milestones${qs}`);
}

/** Create a milestone (admin only). */
export function createMilestone(req: CreateMilestoneRequest): Promise<Milestone> {
  return apiRequest<Milestone>('POST', '/api/milestones', req);
}

/** Update a milestone (admin only). */
export function updateMilestone(id: number, req: UpdateMilestoneRequest): Promise<Milestone> {
  return apiRequest<Milestone>('PATCH', `/api/milestones/${id}`, req);
}

/** Delete a milestone (admin only; fails if it has tickets). */
export function deleteMilestone(id: number): Promise<void> {
  return apiRequest<void>('DELETE', `/api/milestones/${id}`);
}
