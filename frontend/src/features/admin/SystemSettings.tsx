import { usePageHeader } from '../../components/layout/usePageHeader';

/** System settings and configuration panel. */
export default function SystemSettings() {
  usePageHeader({ title: 'System Settings', breadcrumb: ['Admin'] });
  return <div>System Settings</div>;
}
