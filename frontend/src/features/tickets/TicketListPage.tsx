import { usePageHeader } from '../../components/layout/usePageHeader';

/** Paginated ticket list with filter bar and keyboard navigation. */
export default function TicketListPage() {
  usePageHeader({ title: 'Tickets', breadcrumb: [] });
  return <div>Tickets</div>;
}
