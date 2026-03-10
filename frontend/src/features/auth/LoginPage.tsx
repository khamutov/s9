import { type FormEvent, useState } from 'react';
import { Navigate, useNavigate } from 'react-router';
import { ApiError } from '../../api/client';
import { useAuth } from './useAuth';
import styles from './LoginPage.module.css';

/**
 * Login page with username/password form.
 *
 * Shows an OIDC "Sign in with SSO" link that redirects to the backend
 * OIDC authorize endpoint. The backend returns 404 if OIDC is not
 * configured, which is handled gracefully by the browser.
 */
export default function LoginPage() {
  const { user, isLoading, login } = useAuth();
  const navigate = useNavigate();

  const [loginStr, setLoginStr] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  // Already authenticated — redirect to tickets.
  if (!isLoading && user) {
    return <Navigate to="/tickets" replace />;
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError('');
    setSubmitting(true);

    try {
      await login(loginStr, password);
      navigate('/tickets', { replace: true });
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        setError('Invalid login or password.');
      } else {
        setError('Something went wrong. Please try again.');
      }
    } finally {
      setSubmitting(false);
    }
  }

  if (isLoading) return null;

  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <div className={styles.logo}>S9</div>
        <div className={styles.subtitle}>Sign in to your account</div>

        {error && <div className={styles.error}>{error}</div>}

        <form className={styles.form} onSubmit={handleSubmit}>
          <div className={styles.field}>
            <label className={styles.label} htmlFor="login">
              Username
            </label>
            <input
              id="login"
              className={styles.input}
              type="text"
              autoComplete="username"
              autoFocus
              required
              value={loginStr}
              onChange={(e) => setLoginStr(e.target.value)}
              placeholder="username"
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label} htmlFor="password">
              Password
            </label>
            <input
              id="password"
              className={styles.input}
              type="password"
              autoComplete="current-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="password"
            />
          </div>

          <button className={styles.submitBtn} type="submit" disabled={submitting}>
            {submitting ? 'Signing in\u2026' : 'Sign in'}
          </button>
        </form>

        <div className={styles.divider}>
          <span className={styles.dividerText}>or</span>
        </div>

        <a className={styles.oidcBtn} href="/api/auth/oidc/authorize">
          Sign in with SSO
        </a>
      </div>
    </div>
  );
}
