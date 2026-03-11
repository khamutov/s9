import { useState, type FormEvent } from 'react';
import { useMutation } from '@tanstack/react-query';
import { useAuth } from '../auth/useAuth';
import { updateUser, setPassword } from '../../api/users';
import { ApiError } from '../../api/client';
import type { UpdateUserRequest } from '../../api/types';
import styles from './AccountPage.module.css';

interface ProfileErrors {
  display_name?: string;
  email?: string;
  server?: string;
}

interface PasswordErrors {
  current_password?: string;
  new_password?: string;
  confirm_password?: string;
  server?: string;
}

/** Account page for editing profile information and changing password. */
export default function AccountPage() {
  const { user, refreshUser } = useAuth();

  // Profile form
  const [displayName, setDisplayName] = useState(user?.display_name ?? '');
  const [email, setEmail] = useState(user?.email ?? '');
  const [profileErrors, setProfileErrors] = useState<ProfileErrors>({});
  const [profileSuccess, setProfileSuccess] = useState(false);

  // Password form
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [passwordErrors, setPasswordErrors] = useState<PasswordErrors>({});
  const [passwordSuccess, setPasswordSuccess] = useState(false);

  const profileMutation = useMutation({
    mutationFn: (req: UpdateUserRequest) => updateUser(user!.id, req),
    onSuccess: async () => {
      await refreshUser();
      setProfileSuccess(true);
      setProfileErrors({});
      setTimeout(() => setProfileSuccess(false), 3000);
    },
    onError: (err: Error) => {
      setProfileSuccess(false);
      if (err instanceof ApiError && err.details) {
        setProfileErrors({ ...err.details });
      } else {
        setProfileErrors({ server: err.message || 'Failed to update profile.' });
      }
    },
  });

  const passwordMutation = useMutation({
    mutationFn: ({
      id,
      req,
    }: {
      id: number;
      req: { current_password?: string; new_password: string };
    }) => setPassword(id, req),
    onSuccess: () => {
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
      setPasswordSuccess(true);
      setPasswordErrors({});
      setTimeout(() => setPasswordSuccess(false), 3000);
    },
    onError: (err: Error) => {
      setPasswordSuccess(false);
      if (err instanceof ApiError && err.details) {
        setPasswordErrors({ ...err.details });
      } else {
        setPasswordErrors({ server: err.message || 'Failed to change password.' });
      }
    },
  });

  function handleProfileSubmit(e: FormEvent) {
    e.preventDefault();
    const errs: ProfileErrors = {};
    if (!displayName.trim()) errs.display_name = 'Display name is required';
    if (!email.trim()) errs.email = 'Email is required';
    if (Object.keys(errs).length > 0) {
      setProfileErrors(errs);
      return;
    }
    setProfileSuccess(false);
    profileMutation.mutate({
      display_name: displayName.trim(),
      email: email.trim(),
    });
  }

  function handlePasswordSubmit(e: FormEvent) {
    e.preventDefault();
    const errs: PasswordErrors = {};
    if (!currentPassword.trim()) errs.current_password = 'Current password is required';
    if (!newPassword) errs.new_password = 'New password is required';
    else if (newPassword.length < 8) errs.new_password = 'Password must be at least 8 characters';
    if (newPassword !== confirmPassword) errs.confirm_password = 'Passwords do not match';
    if (Object.keys(errs).length > 0) {
      setPasswordErrors(errs);
      return;
    }
    setPasswordSuccess(false);
    passwordMutation.mutate({
      id: user!.id,
      req: { current_password: currentPassword, new_password: newPassword },
    });
  }

  if (!user) return null;

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h1>Account</h1>
      </div>

      <div className={styles.sections}>
        {/* Profile Section */}
        <section className={styles.section}>
          <h2 className={styles.sectionTitle}>Profile</h2>
          {profileErrors.server && <div className={styles.serverError}>{profileErrors.server}</div>}
          {profileSuccess && <div className={styles.successMsg}>Profile updated.</div>}
          <form onSubmit={handleProfileSubmit} noValidate>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="account-login">
                Login
              </label>
              <div className={styles.readonlyValue}>{user.login}</div>
            </div>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="account-role">
                Role
              </label>
              <div className={styles.readonlyValue}>{user.role}</div>
            </div>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="account-display-name">
                Display Name <span className={styles.required}>*</span>
              </label>
              <input
                id="account-display-name"
                className={`${styles.formInput} ${profileErrors.display_name ? styles.formInputError : ''}`}
                type="text"
                value={displayName}
                onChange={(e) => {
                  setDisplayName(e.target.value);
                  if (profileErrors.display_name)
                    setProfileErrors((p) => ({ ...p, display_name: undefined }));
                }}
              />
              {profileErrors.display_name && (
                <div className={styles.formError}>{profileErrors.display_name}</div>
              )}
            </div>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="account-email">
                Email <span className={styles.required}>*</span>
              </label>
              <input
                id="account-email"
                className={`${styles.formInput} ${profileErrors.email ? styles.formInputError : ''}`}
                type="email"
                value={email}
                onChange={(e) => {
                  setEmail(e.target.value);
                  if (profileErrors.email) setProfileErrors((p) => ({ ...p, email: undefined }));
                }}
              />
              {profileErrors.email && <div className={styles.formError}>{profileErrors.email}</div>}
            </div>
            <div className={styles.formActions}>
              <button
                type="submit"
                className={styles.btnPrimary}
                disabled={profileMutation.isPending}
              >
                {profileMutation.isPending ? 'Saving...' : 'Save Profile'}
              </button>
            </div>
          </form>
        </section>

        {/* Password Section */}
        <section className={styles.section}>
          <h2 className={styles.sectionTitle}>Change Password</h2>
          {passwordErrors.server && (
            <div className={styles.serverError}>{passwordErrors.server}</div>
          )}
          {passwordSuccess && <div className={styles.successMsg}>Password changed.</div>}
          <form onSubmit={handlePasswordSubmit} noValidate>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="account-current-password">
                Current Password <span className={styles.required}>*</span>
              </label>
              <input
                id="account-current-password"
                className={`${styles.formInput} ${passwordErrors.current_password ? styles.formInputError : ''}`}
                type="password"
                value={currentPassword}
                onChange={(e) => {
                  setCurrentPassword(e.target.value);
                  if (passwordErrors.current_password)
                    setPasswordErrors((p) => ({ ...p, current_password: undefined }));
                }}
              />
              {passwordErrors.current_password && (
                <div className={styles.formError}>{passwordErrors.current_password}</div>
              )}
            </div>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="account-new-password">
                New Password <span className={styles.required}>*</span>
              </label>
              <input
                id="account-new-password"
                className={`${styles.formInput} ${passwordErrors.new_password ? styles.formInputError : ''}`}
                type="password"
                value={newPassword}
                onChange={(e) => {
                  setNewPassword(e.target.value);
                  if (passwordErrors.new_password)
                    setPasswordErrors((p) => ({ ...p, new_password: undefined }));
                }}
              />
              {passwordErrors.new_password && (
                <div className={styles.formError}>{passwordErrors.new_password}</div>
              )}
            </div>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="account-confirm-password">
                Confirm New Password <span className={styles.required}>*</span>
              </label>
              <input
                id="account-confirm-password"
                className={`${styles.formInput} ${passwordErrors.confirm_password ? styles.formInputError : ''}`}
                type="password"
                value={confirmPassword}
                onChange={(e) => {
                  setConfirmPassword(e.target.value);
                  if (passwordErrors.confirm_password)
                    setPasswordErrors((p) => ({ ...p, confirm_password: undefined }));
                }}
              />
              {passwordErrors.confirm_password && (
                <div className={styles.formError}>{passwordErrors.confirm_password}</div>
              )}
            </div>
            <div className={styles.formActions}>
              <button
                type="submit"
                className={styles.btnPrimary}
                disabled={passwordMutation.isPending}
              >
                {passwordMutation.isPending ? 'Changing...' : 'Change Password'}
              </button>
            </div>
          </form>
        </section>
      </div>
    </div>
  );
}
