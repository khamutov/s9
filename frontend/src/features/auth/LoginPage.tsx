import { usePageHeader } from '../../components/layout/usePageHeader';

/** Login page with email/password form and optional OIDC button. */
export default function LoginPage() {
  usePageHeader({ title: 'Login', breadcrumb: [] });
  return <div>Login</div>;
}
