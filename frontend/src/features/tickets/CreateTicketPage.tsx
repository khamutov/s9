import { usePageHeader } from '../../components/layout/usePageHeader';

/** Create ticket form with title, description, type, priority, component, owner, CC, milestones. */
export default function CreateTicketPage() {
  usePageHeader({ title: 'Create Ticket', breadcrumb: ['Tickets'] });
  return <div>Create Ticket</div>;
}
