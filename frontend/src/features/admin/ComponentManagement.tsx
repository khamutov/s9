import { useState, useMemo, type FormEvent } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { usePageHeader } from '../../components/layout/usePageHeader';
import { useAuth } from '../auth/useAuth';
import { useComponents } from '../components/useComponents';
import { useUsers } from './useUsers';
import { createComponent, updateComponent, deleteComponent } from '../../api/components';
import { ApiError } from '../../api/client';
import type { Component, CreateComponentRequest, UpdateComponentRequest } from '../../api/types';
import styles from './ComponentManagement.module.css';

type ModalMode = 'closed' | 'create' | 'edit' | 'delete';

interface FormErrors {
  name?: string;
  slug?: string;
  owner_id?: string;
  server?: string;
}

/** Admin component management with create, edit, reparent, and delete. */
export default function ComponentManagement() {
  usePageHeader({ title: 'Component Management', breadcrumb: ['Admin'] });
  const { user: currentUser } = useAuth();
  const queryClient = useQueryClient();

  const { data: componentsData, isLoading, error } = useComponents();
  const { data: usersData } = useUsers();

  const [modal, setModal] = useState<ModalMode>('closed');
  const [editingComponent, setEditingComponent] = useState<Component | null>(null);
  const [errors, setErrors] = useState<FormErrors>({});

  // Form fields
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [parentId, setParentId] = useState('');
  const [ownerId, setOwnerId] = useState('');

  const components = useMemo(() => componentsData?.items ?? [], [componentsData]);
  const users = useMemo(() => usersData?.items ?? [], [usersData]);

  const createMutation = useMutation({
    mutationFn: (req: CreateComponentRequest) => createComponent(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['components'] });
      closeModal();
    },
    onError: handleMutationError,
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, req }: { id: number; req: UpdateComponentRequest }) =>
      updateComponent(id, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['components'] });
      closeModal();
    },
    onError: handleMutationError,
  });

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteComponent(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['components'] });
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
    setName('');
    setSlug('');
    setParentId('');
    setOwnerId('');
    setErrors({});
    setModal('create');
  }

  function openEdit(c: Component) {
    setEditingComponent(c);
    setName(c.name);
    setSlug(c.slug ?? '');
    setParentId(c.parent_id != null ? String(c.parent_id) : '');
    setOwnerId(String(c.owner.id));
    setErrors({});
    setModal('edit');
  }

  function openDelete(c: Component) {
    setEditingComponent(c);
    setErrors({});
    setModal('delete');
  }

  function closeModal() {
    setModal('closed');
    setEditingComponent(null);
    setErrors({});
  }

  function handleCreate(e: FormEvent) {
    e.preventDefault();
    const next: FormErrors = {};
    if (!name.trim()) next.name = 'Name is required';
    if (!ownerId) next.owner_id = 'Owner is required';
    if (Object.keys(next).length > 0) {
      setErrors(next);
      return;
    }
    const req: CreateComponentRequest = {
      name: name.trim(),
      owner_id: Number(ownerId),
    };
    if (slug.trim()) req.slug = slug.trim();
    if (parentId) req.parent_id = Number(parentId);
    createMutation.mutate(req);
  }

  function handleEdit(e: FormEvent) {
    e.preventDefault();
    if (!editingComponent) return;
    const next: FormErrors = {};
    if (!name.trim()) next.name = 'Name is required';
    if (Object.keys(next).length > 0) {
      setErrors(next);
      return;
    }
    const req: UpdateComponentRequest = {
      name: name.trim(),
      owner_id: Number(ownerId),
    };
    if (slug.trim()) {
      req.slug = slug.trim();
    } else {
      req.slug = null;
    }
    req.parent_id = parentId ? Number(parentId) : null;
    updateMutation.mutate({ id: editingComponent.id, req });
  }

  function handleDelete() {
    if (!editingComponent) return;
    deleteMutation.mutate(editingComponent.id);
  }

  if (currentUser?.role !== 'admin') {
    return (
      <div className={styles.accessDenied}>
        You need administrator privileges to access this page.
      </div>
    );
  }

  const isPending =
    createMutation.isPending || updateMutation.isPending || deleteMutation.isPending;

  return (
    <div>
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.breadcrumb}>Administration</div>
          <h1>
            Components {!isLoading && <span className={styles.pageCount}>{components.length}</span>}
          </h1>
        </div>
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
          Add Component
        </button>
      </div>

      {isLoading ? (
        <div className={styles.emptyState}>Loading components…</div>
      ) : error ? (
        <div className={styles.errorState}>Failed to load components. Please try again.</div>
      ) : components.length === 0 ? (
        <div className={styles.emptyState}>No components found.</div>
      ) : (
        <div className={styles.tableWrap}>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Name</th>
                <th>Path</th>
                <th>Slug</th>
                <th>Owner</th>
                <th>Tickets</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {components.map((c) => (
                <tr key={c.id}>
                  <td className={styles.nameCell}>{c.name}</td>
                  <td className={styles.pathCell}>{c.path}</td>
                  <td className={styles.slugCell}>{c.effective_slug ?? '\u2014'}</td>
                  <td>{c.owner.display_name}</td>
                  <td className={styles.countCell}>{c.ticket_count}</td>
                  <td className={styles.actionsCell}>
                    <button className={styles.actionBtn} onClick={() => openEdit(c)}>
                      Edit
                    </button>
                    <button
                      className={`${styles.actionBtn} ${styles.deleteBtn}`}
                      onClick={() => openDelete(c)}
                    >
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Create Component Modal */}
      {modal === 'create' && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Create Component</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <form onSubmit={handleCreate} noValidate>
              <div className={styles.modalBody}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="comp-name">
                    Name <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="comp-name"
                    className={`${styles.formInput} ${errors.name ? styles.formInputError : ''}`}
                    type="text"
                    value={name}
                    onChange={(e) => {
                      setName(e.target.value);
                      if (errors.name) setErrors((p) => ({ ...p, name: undefined }));
                    }}
                    autoFocus
                  />
                  {errors.name && <div className={styles.formError}>{errors.name}</div>}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="comp-slug">
                    Slug <span className={styles.optional}>optional</span>
                  </label>
                  <input
                    id="comp-slug"
                    className={`${styles.formInput} ${errors.slug ? styles.formInputError : ''}`}
                    type="text"
                    value={slug}
                    onChange={(e) => setSlug(e.target.value)}
                    placeholder="e.g. PLAT"
                  />
                  {errors.slug && <div className={styles.formError}>{errors.slug}</div>}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="comp-parent">
                    Parent <span className={styles.optional}>optional</span>
                  </label>
                  <div className={styles.formSelectWrap}>
                    <select
                      id="comp-parent"
                      className={styles.formSelect}
                      value={parentId}
                      onChange={(e) => setParentId(e.target.value)}
                    >
                      <option value="">None (root)</option>
                      {components.map((c) => (
                        <option key={c.id} value={c.id}>
                          {c.path}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="comp-owner">
                    Owner <span className={styles.required}>*</span>
                  </label>
                  <div className={styles.formSelectWrap}>
                    <select
                      id="comp-owner"
                      className={`${styles.formSelect} ${errors.owner_id ? styles.formInputError : ''}`}
                      value={ownerId}
                      onChange={(e) => {
                        setOwnerId(e.target.value);
                        if (errors.owner_id) setErrors((p) => ({ ...p, owner_id: undefined }));
                      }}
                    >
                      <option value="">Select owner...</option>
                      {users.map((u) => (
                        <option key={u.id} value={u.id}>
                          {u.display_name}
                        </option>
                      ))}
                    </select>
                  </div>
                  {errors.owner_id && <div className={styles.formError}>{errors.owner_id}</div>}
                </div>
              </div>
              <div className={styles.modalActions}>
                <button type="submit" className={styles.btnPrimary} disabled={isPending}>
                  {createMutation.isPending ? 'Creating…' : 'Create Component'}
                </button>
                <button type="button" className={styles.btnGhost} onClick={closeModal}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Edit Component Modal */}
      {modal === 'edit' && editingComponent && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Edit Component</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <form onSubmit={handleEdit} noValidate>
              <div className={styles.modalBody}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-comp-name">
                    Name <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="edit-comp-name"
                    className={`${styles.formInput} ${errors.name ? styles.formInputError : ''}`}
                    type="text"
                    value={name}
                    onChange={(e) => {
                      setName(e.target.value);
                      if (errors.name) setErrors((p) => ({ ...p, name: undefined }));
                    }}
                    autoFocus
                  />
                  {errors.name && <div className={styles.formError}>{errors.name}</div>}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-comp-slug">
                    Slug <span className={styles.optional}>optional</span>
                  </label>
                  <input
                    id="edit-comp-slug"
                    className={`${styles.formInput} ${errors.slug ? styles.formInputError : ''}`}
                    type="text"
                    value={slug}
                    onChange={(e) => setSlug(e.target.value)}
                    placeholder="e.g. PLAT"
                  />
                  {errors.slug && <div className={styles.formError}>{errors.slug}</div>}
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-comp-parent">
                    Parent
                  </label>
                  <div className={styles.formSelectWrap}>
                    <select
                      id="edit-comp-parent"
                      className={styles.formSelect}
                      value={parentId}
                      onChange={(e) => setParentId(e.target.value)}
                    >
                      <option value="">None (root)</option>
                      {components
                        .filter((c) => c.id !== editingComponent.id)
                        .map((c) => (
                          <option key={c.id} value={c.id}>
                            {c.path}
                          </option>
                        ))}
                    </select>
                  </div>
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-comp-owner">
                    Owner <span className={styles.required}>*</span>
                  </label>
                  <div className={styles.formSelectWrap}>
                    <select
                      id="edit-comp-owner"
                      className={styles.formSelect}
                      value={ownerId}
                      onChange={(e) => setOwnerId(e.target.value)}
                    >
                      {users.map((u) => (
                        <option key={u.id} value={u.id}>
                          {u.display_name}
                        </option>
                      ))}
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

      {/* Delete Confirmation Modal */}
      {modal === 'delete' && editingComponent && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Delete Component</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <div className={styles.modalBody}>
              <p className={styles.deleteWarning}>
                Are you sure you want to delete <strong>{editingComponent.name}</strong>? This
                action cannot be undone.
              </p>
              {editingComponent.ticket_count > 0 && (
                <p className={styles.deleteNote}>
                  This component has {editingComponent.ticket_count} assigned ticket
                  {editingComponent.ticket_count !== 1 ? 's' : ''} and cannot be deleted.
                </p>
              )}
            </div>
            <div className={styles.modalActions}>
              <button
                className={styles.btnDanger}
                onClick={handleDelete}
                disabled={isPending || editingComponent.ticket_count > 0}
              >
                {deleteMutation.isPending ? 'Deleting…' : 'Delete'}
              </button>
              <button type="button" className={styles.btnGhost} onClick={closeModal}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
