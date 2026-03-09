import { useParams } from 'react-router';
import { usePageHeader } from '../../components/layout/usePageHeader';

/** Milestone detail view with ticket breakdown and progress. */
export default function MilestoneDetailPage() {
  const { id } = useParams<{ id: string }>();
  usePageHeader({ title: `Milestone #${id ?? ''}`, breadcrumb: ['Milestones'] });
  return <div>Milestone #{id}</div>;
}
