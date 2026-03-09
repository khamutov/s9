import { usePageHeader } from '../../components/layout/usePageHeader';

/** Admin component tree management with create, rename, reparent, delete. */
export default function ComponentManagement() {
  usePageHeader({ title: 'Component Management' });
  return <div>Component Management</div>;
}
