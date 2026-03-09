import { usePageHeader } from '../../components/layout/usePageHeader';

/** Hierarchical component tree view with ticket counts. */
export default function ComponentTreePage() {
  usePageHeader({ title: 'Components', breadcrumb: [] });
  return <div>Components</div>;
}
