import { useState, useMemo, type FormEvent } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { usePageHeader } from '../../components/layout/usePageHeader';
import { useAuth } from '../auth/useAuth';
import { listUsers, createUser, updateUser, setPassword } from '../../api/users';
import { ApiError } from '../../api/client';
import type { User, UserRole, CreateUserRequest, UpdateUserRequest } from '../../api/types';
import styles from './UserManagement.module.css';

type ModalMode = 'closed' | 'create' | 'edit' | 'password';

interface FormErrors {
  login?: string;
  display_name?: string;
  email?: string;
  password?: string;
  new_password?: string;
  server?: string;
}

/** Admin user management with create, edit, and deactivate. */
export default function UserManagement() {
  usePageHeader({ title: 'User Management', breadcrumb: ['Admin'] });
  const { user: currentUser } = useAuth();
  const queryClient = useQueryClient();

  const [showInactive, setShowInactive] = useState(false);
  const [modal, setModal] = useState<ModalMode>('closed');
  const [editingUser, setEditingUser] = useState<User | null>(null);
  const [errors, setErrors] = useState<FormErrors>({});

  // Form fields
  const [login, setLogin] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [email, setEmail] = useState('');
  const [role, setRole] = useState<UserRole>('user');
  const [password, setPassword_] = useState('');
  const [newPassword, setNewPassword] = useState('');

  const { data, isLoading, error } = useQuery({
    queryKey: ['users', { includeInactive: showInactive }],
    queryFn: () => listUsers(showInactive),
  });

  const users = useMemo(() => data?.items ?? [], [data]);

  const createMutation = useMutation({
    mutationFn: (req: CreateUserRequest) => createUser(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] });
      closeModal();
    },
    onError: handleMutationError,
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, req }: { id: number; req: UpdateUserRequest }) => updateUser(id, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] });
      closeModal();
    },
    onError: handleMutationError,
  });

  const passwordMutation = useMutation({
    mutationFn: ({ id, pw }: { id: number; pw: string }) => setPassword(id, { new_password: pw }),
    onSuccess: () => {
      closeModal();
    },
    onError: handleMutationError,
  });

  function handleMutationError(err: Error) {
    if (err instanceof ApiError && err.details) {
      setErrors({ ...err.details });
    } else {
      setErrors({ server: err.message || 'Operation failed. Please try again.' });
    }
  }

  function openCreate() {
    setLogin('');
    setDisplayName('');
    setEmail('');
    setRole('user');
    setPassword_('');
    setErrors({});
    setModal('create');
  }

  function openEdit(u: User) {
    setEditingUser(u);
    setDisplayName(u.display_name);
    setEmail(u.email);
    setRole(u.role);
    setErrors({});
    setModal('edit');
  }

  function openPassword(u: User) {
    setEditingUser(u);
    setNewPassword('');
    setErrors({});
    setModal('password');
  }

  function closeModal() {
    setModal('closed');
    setEditingUser(null);
    setErrors({});
  }

  function handleCreate(e: FormEvent) {
    e.preventDefault();
    const next: FormErrors = {};
    if (!login.trim()) next.login = 'Login is required';
    if (!displayName.trim()) next.display_name = 'Display name is required';
    if (!email.trim()) next.email = 'Email is required';
    if (Object.keys(next).length > 0) {
      setErrors(next);
      return;
    }
    const req: CreateUserRequest = {
      login: login.trim(),
      display_name: displayName.trim(),
      email: email.trim(),
      role,
    };
    if (password.trim()) req.password = password.trim();
    createMutation.mutate(req);
  }

  function handleEdit(e: FormEvent) {
    e.preventDefault();
    if (!editingUser) return;
    const next: FormErrors = {};
    if (!displayName.trim()) next.display_name = 'Display name is required';
    if (!email.trim()) next.email = 'Email is required';
    if (Object.keys(next).length > 0) {
      setErrors(next);
      return;
    }
    const req: UpdateUserRequest = {
      display_name: displayName.trim(),
      email: email.trim(),
      role,
    };
    updateMutation.mutate({ id: editingUser.id, req });
  }

  function handleSetPassword(e: FormEvent) {
    e.preventDefault();
    if (!editingUser) return;
    if (!newPassword.trim()) {
      setErrors({ new_password: 'Password is required' });
      return;
    }
    if (newPassword.length < 8) {
      setErrors({ new_password: 'Password must be at least 8 characters' });
      return;
    }
    passwordMutation.mutate({ id: editingUser.id, pw: newPassword });
  }

  function handleToggleActive(u: User) {
    updateMutation.mutate({
      id: u.id,
      req: { is_active: !u.is_active },
    });
  }

  if (currentUser?.role !== 'admin') {
    return (
      <div className={styles.accessDenied}>
        You need administrator privileges to access this page.
      </div>
    );
  }

  const isPending =
    createMutation.isPending || updateMutation.isPending || passwordMutation.isPending;

  return (
    <div>
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.breadcrumb}>Administration</div>
          <h1>Users {!isLoading && <span className={styles.pageCount}>{users.length}</span>}</h1>
        </div>
        <div className={styles.headerActions}>
          <label className={styles.toggleLabel}>
            <input
              type="checkbox"
              checked={showInactive}
              onChange={(e) => setShowInactive(e.target.checked)}
              className={styles.toggleInput}
            />
            Show inactive
          </label>
          <button className={styles.btnPrimary} onClick={openCreate}>
            <svg
              width="14"
              height="14"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              strokeLinecap="round"
            >
              <path d="M8 3v10M3 8h10" />
            </svg>
            Add User
          </button>
        </div>
      </div>

      {isLoading ? (
        <div className={styles.emptyState}>Loading users…</div>
      ) : error ? (
        <div className={styles.errorState}>Failed to load users. Please try again.</div>
      ) : users.length === 0 ? (
        <div className={styles.emptyState}>No users found.</div>
      ) : (
        <div className={styles.tableWrap}>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Login</th>
                <th>Display Name</th>
                <th>Email</th>
                <th>Role</th>
                <th>Status</th>
                <th>Auth</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr key={u.id} className={!u.is_active ? styles.inactiveRow : undefined}>
                  <td className={styles.loginCell}>{u.login}</td>
                  <td>{u.display_name}</td>
                  <td className={styles.emailCell}>{u.email}</td>
                  <td>
                    <span
                      className={`${styles.roleBadge} ${u.role === 'admin' ? styles.roleAdmin : styles.roleUser}`}
                    >
                      {u.role}
                    </span>
                  </td>
                  <td>
                    <span
                      className={`${styles.statusBadge} ${u.is_active ? styles.statusActive : styles.statusInactive}`}
                    >
                      {u.is_active ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                  <td className={styles.authCell}>
                    {u.has_password && <span className={styles.authTag}>password</span>}
                    {u.has_oidc && <span className={styles.authTag}>oidc</span>}
                  </td>
                  <td className={styles.actionsCell}>
                    <button
                      className={styles.actionBtn}
                      onClick={() => openEdit(u)}
                      title="Edit user"
                    >
                      Edit
                    </button>
                    <button
                      className={styles.actionBtn}
                      onClick={() => openPassword(u)}
                      title="Set password"
                    >
                      Password
                    </button>
                    <button
                      className={`${styles.actionBtn} ${u.is_active ? styles.deactivateBtn : styles.activateBtn}`}
                      onClick={() => handleToggleActive(u)}
                      title={u.is_active ? 'Deactivate user' : 'Activate user'}
                    >
                      {u.is_active ? 'Deactivate' : 'Activate'}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Create User Modal */}
      {modal === 'create' && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Create User</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <form onSubmit={handleCreate} noValidate>
              <div className={styles.modalBody}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="user-login">
                    Login <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="user-login"
                    className={`${styles.formInput} ${errors.login ? styles.formInputError : ''}`}
                    type="text"
                    value={login}
                    onChange={(e) => {
                      setLogin(e.target.value);
                      if (errors.login) setErrors((p) => ({ ...p, login: undefined }));
                    }}
                    autoFocus
                  />
                  {errors.login && <div className={styles.formError}>{errors.login}</div>}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="user-display-name">
                    Display Name <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="user-display-name"
                    className={`${styles.formInput} ${errors.display_name ? styles.formInputError : ''}`}
                    type="text"
                    value={displayName}
                    onChange={(e) => {
                      setDisplayName(e.target.value);
                      if (errors.display_name)
                        setErrors((p) => ({ ...p, display_name: undefined }));
                    }}
                  />
                  {errors.display_name && (
                    <div className={styles.formError}>{errors.display_name}</div>
                  )}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="user-email">
                    Email <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="user-email"
                    className={`${styles.formInput} ${errors.email ? styles.formInputError : ''}`}
                    type="email"
                    value={email}
                    onChange={(e) => {
                      setEmail(e.target.value);
                      if (errors.email) setErrors((p) => ({ ...p, email: undefined }));
                    }}
                  />
                  {errors.email && <div className={styles.formError}>{errors.email}</div>}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="user-role">
                    Role
                  </label>
                  <div className={styles.formSelectWrap}>
                    <select
                      id="user-role"
                      className={styles.formSelect}
                      value={role}
                      onChange={(e) => setRole(e.target.value as UserRole)}
                    >
                      <option value="user">User</option>
                      <option value="admin">Admin</option>
                    </select>
                  </div>
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="user-password">
                    Password <span className={styles.optional}>optional</span>
                  </label>
                  <input
                    id="user-password"
                    className={`${styles.formInput} ${errors.password ? styles.formInputError : ''}`}
                    type="password"
                    value={password}
                    onChange={(e) => setPassword_(e.target.value)}
                    placeholder="Leave blank for OIDC-only"
                  />
                  {errors.password && <div className={styles.formError}>{errors.password}</div>}
                </div>
              </div>
              <div className={styles.modalActions}>
                <button type="submit" className={styles.btnPrimary} disabled={isPending}>
                  {createMutation.isPending ? 'Creating…' : 'Create User'}
                </button>
                <button type="button" className={styles.btnGhost} onClick={closeModal}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Edit User Modal */}
      {modal === 'edit' && editingUser && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Edit User</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <form onSubmit={handleEdit} noValidate>
              <div className={styles.modalBody}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel}>Login</label>
                  <div className={styles.readonlyValue}>{editingUser.login}</div>
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-display-name">
                    Display Name <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="edit-display-name"
                    className={`${styles.formInput} ${errors.display_name ? styles.formInputError : ''}`}
                    type="text"
                    value={displayName}
                    onChange={(e) => {
                      setDisplayName(e.target.value);
                      if (errors.display_name)
                        setErrors((p) => ({ ...p, display_name: undefined }));
                    }}
                    autoFocus
                  />
                  {errors.display_name && (
                    <div className={styles.formError}>{errors.display_name}</div>
                  )}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-email">
                    Email <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="edit-email"
                    className={`${styles.formInput} ${errors.email ? styles.formInputError : ''}`}
                    type="email"
                    value={email}
                    onChange={(e) => {
                      setEmail(e.target.value);
                      if (errors.email) setErrors((p) => ({ ...p, email: undefined }));
                    }}
                  />
                  {errors.email && <div className={styles.formError}>{errors.email}</div>}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-role">
                    Role
                  </label>
                  <div className={styles.formSelectWrap}>
                    <select
                      id="edit-role"
                      className={styles.formSelect}
                      value={role}
                      onChange={(e) => setRole(e.target.value as UserRole)}
                    >
                      <option value="user">User</option>
                      <option value="admin">Admin</option>
                    </select>
                  </div>
                </div>
              </div>
              <div className={styles.modalActions}>
                <button type="submit" className={styles.btnPrimary} disabled={isPending}>
                  {updateMutation.isPending ? 'Saving…' : 'Save Changes'}
                </button>
                <button type="button" className={styles.btnGhost} onClick={closeModal}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Set Password Modal */}
      {modal === 'password' && editingUser && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Set Password</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            <div className={styles.modalSubtitle}>
              Setting password for <strong>{editingUser.display_name}</strong>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <form onSubmit={handleSetPassword} noValidate>
              <div className={styles.modalBody}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="new-password">
                    New Password <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="new-password"
                    className={`${styles.formInput} ${errors.new_password ? styles.formInputError : ''}`}
                    type="password"
                    value={newPassword}
                    onChange={(e) => {
                      setNewPassword(e.target.value);
                      if (errors.new_password)
                        setErrors((p) => ({ ...p, new_password: undefined }));
                    }}
                    autoFocus
                  />
                  {errors.new_password && (
                    <div className={styles.formError}>{errors.new_password}</div>
                  )}
                </div>
              </div>
              <div className={styles.modalActions}>
                <button type="submit" className={styles.btnPrimary} disabled={isPending}>
                  {passwordMutation.isPending ? 'Setting…' : 'Set Password'}
                </button>
                <button type="button" className={styles.btnGhost} onClick={closeModal}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
