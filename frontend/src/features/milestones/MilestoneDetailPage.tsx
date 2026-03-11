import { useParams } from 'react-router';

/** Milestone detail view with ticket breakdown and progress. */
export default function MilestoneDetailPage() {
  const { id } = useParams<{ id: string }>();
  return <div>Milestone #{id}</div>;
}
