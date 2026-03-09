import { usePageHeader } from '../../components/layout/usePageHeader';

/** Paginated ticket list with filter bar and keyboard navigation. */
export default function TicketListPage() {
  usePageHeader({ title: 'Tickets' });
  return <div>Tickets</div>;
}
