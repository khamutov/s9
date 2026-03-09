import { usePageHeader } from '../../components/layout/usePageHeader';

/** Admin user management with create, edit, and deactivate. */
export default function UserManagement() {
  usePageHeader({ title: 'User Management', breadcrumb: ['Admin'] });
  return <div>User Management</div>;
}
