import { useParams } from 'react-router';
import { usePageHeader } from '../../components/layout/usePageHeader';

/** Ticket detail view with metadata sidebar, description, and comment thread. */
export default function TicketDetailPage() {
  const { id } = useParams<{ id: string }>();
  usePageHeader({ title: `Ticket #${id ?? ''}`, breadcrumb: ['Tickets'] });
  return <div>Ticket #{id}</div>;
}
