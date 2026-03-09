import { usePageHeader } from '../../components/layout/usePageHeader';

/** Admin dashboard landing page. */
export default function AdminPanel() {
  usePageHeader({ title: 'Admin', breadcrumb: [] });
  return <div>Admin Panel</div>;
}
