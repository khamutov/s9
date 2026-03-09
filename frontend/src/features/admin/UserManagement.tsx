import { usePageHeader } from '../../components/layout/usePageHeader';

/** Admin user management with create, edit, and deactivate. */
export default function UserManagement() {
  usePageHeader({ title: 'User Management' });
  return <div>User Management</div>;
}
