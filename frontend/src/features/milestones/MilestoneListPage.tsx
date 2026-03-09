import { usePageHeader } from '../../components/layout/usePageHeader';

/** Milestone list with progress bars and status filter. */
export default function MilestoneListPage() {
  usePageHeader({ title: 'Milestones', breadcrumb: [] });
  return <div>Milestones</div>;
}
