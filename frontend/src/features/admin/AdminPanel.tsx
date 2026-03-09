import { usePageHeader } from '../../components/layout/usePageHeader';

/** Admin dashboard landing page. */
export default function AdminPanel() {
  usePageHeader({ title: 'Admin' });
  return <div>Admin Panel</div>;
}
