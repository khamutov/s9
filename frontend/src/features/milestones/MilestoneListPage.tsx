import { useState, useMemo, type FormEvent } from 'react';
import { Link } from 'react-router';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useAuth } from '../auth/useAuth';
import { useMilestones } from './useMilestones';
import { createMilestone, updateMilestone, deleteMilestone } from '../../api/milestones';
import { ApiError } from '../../api/client';
import type {
  Milestone,
  MilestoneStatus,
  CreateMilestoneRequest,
  UpdateMilestoneRequest,
} from '../../api/types';
import styles from './MilestoneListPage.module.css';

type ModalMode = 'closed' | 'create' | 'edit' | 'delete';

interface FormErrors {
  name?: string;
  due_date?: string;
  server?: string;
}

/** Format a due date string for display. */
function formatDueDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

/** Check if a due date is in the past. */
function isOverdue(iso: string): boolean {
  return new Date(iso) < new Date();
}

/** Compute the done percentage from milestone stats. */
function donePercent(m: Milestone): number {
  if (m.stats.total === 0) return 0;
  return Math.round((m.stats.done / m.stats.total) * 100);
}

/** Compute segment widths (%) for the progress bar. */
function segmentWidths(m: Milestone): {
  done: number;
  verify: number;
  inProgress: number;
  new: number;
} {
  const t = m.stats.total;
  if (t === 0) return { done: 0, verify: 0, inProgress: 0, new: 0 };
  return {
    done: (m.stats.done / t) * 100,
    verify: (m.stats.verify / t) * 100,
    inProgress: (m.stats.in_progress / t) * 100,
    new: (m.stats.new / t) * 100,
  };
}

const STATUS_COLORS = {
  done: '#5eca7e',
  verify: '#e8b43a',
  in_progress: '#7cb8f7',
  new: '#8c8579',
};

/** Milestone list page with progress cards, filter, and admin CRUD modals. */
export default function MilestoneListPage() {
  const { user: currentUser } = useAuth();
  const queryClient = useQueryClient();
  const { data, isLoading, error } = useMilestones();
  const [filter, setFilter] = useState('');

  const [modal, setModal] = useState<ModalMode>('closed');
  const [editingMilestone, setEditingMilestone] = useState<Milestone | null>(null);
  const [errors, setErrors] = useState<FormErrors>({});

  // Form fields
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [dueDate, setDueDate] = useState('');
  const [status, setStatus] = useState<MilestoneStatus>('open');

  const isAdmin = currentUser?.role === 'admin';

  const milestones = useMemo(() => data?.items ?? [], [data]);

  const filtered = useMemo(() => {
    const q = filter.toLowerCase().trim();
    if (!q) return milestones;
    return milestones.filter(
      (m) =>
        m.name.toLowerCase().includes(q) ||
        (m.description && m.description.toLowerCase().includes(q)),
    );
  }, [milestones, filter]);

  const createMut = useMutation({
    mutationFn: (req: CreateMilestoneRequest) => createMilestone(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['milestones'] });
      closeModal();
    },
    onError: handleMutationError,
  });

  const updateMut = useMutation({
    mutationFn: ({ id, req }: { id: number; req: UpdateMilestoneRequest }) =>
      updateMilestone(id, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['milestones'] });
      closeModal();
    },
    onError: handleMutationError,
  });

  const deleteMut = useMutation({
    mutationFn: (id: number) => deleteMilestone(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['milestones'] });
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
    setDescription('');
    setDueDate('');
    setStatus('open');
    setErrors({});
    setModal('create');
  }

  function openEdit(m: Milestone) {
    setEditingMilestone(m);
    setName(m.name);
    setDescription(m.description ?? '');
    setDueDate(m.due_date ?? '');
    setStatus(m.status);
    setErrors({});
    setModal('edit');
  }

  function openDelete(m: Milestone) {
    setEditingMilestone(m);
    setErrors({});
    setModal('delete');
  }

  function closeModal() {
    setModal('closed');
    setEditingMilestone(null);
    setErrors({});
  }

  function handleCreate(e: FormEvent) {
    e.preventDefault();
    const next: FormErrors = {};
    if (!name.trim()) next.name = 'Name is required';
    if (Object.keys(next).length > 0) {
      setErrors(next);
      return;
    }
    const req: CreateMilestoneRequest = { name: name.trim() };
    if (description.trim()) req.description = description.trim();
    if (dueDate) req.due_date = dueDate;
    if (status !== 'open') req.status = status;
    createMut.mutate(req);
  }

  function handleEdit(e: FormEvent) {
    e.preventDefault();
    if (!editingMilestone) return;
    const next: FormErrors = {};
    if (!name.trim()) next.name = 'Name is required';
    if (Object.keys(next).length > 0) {
      setErrors(next);
      return;
    }
    const req: UpdateMilestoneRequest = {
      name: name.trim(),
      status,
    };
    req.description = description.trim() || null;
    req.due_date = dueDate || null;
    updateMut.mutate({ id: editingMilestone.id, req });
  }

  function handleDelete() {
    if (!editingMilestone) return;
    deleteMut.mutate(editingMilestone.id);
  }

  const isPending = createMut.isPending || updateMut.isPending || deleteMut.isPending;

  return (
    <div>
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.breadcrumb}>All milestones</div>
          <h1>
            Milestones {!isLoading && <span className={styles.pageCount}>{milestones.length}</span>}
          </h1>
        </div>
        {isAdmin && (
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
            Create Milestone
          </button>
        )}
      </div>

      <div className={styles.filterBar}>
        <svg
          className={styles.filterIcon}
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        >
          <circle cx="6.5" cy="6.5" r="4.5" />
          <path d="M10 10l4 4" />
        </svg>
        <input
          className={styles.filterInput}
          type="text"
          placeholder="Filter milestones..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>

      {isLoading ? (
        <div className={styles.emptyState}>Loading milestones…</div>
      ) : error ? (
        <div className={styles.errorState}>Failed to load milestones. Please try again.</div>
      ) : filtered.length === 0 ? (
        <div className={styles.emptyState}>No milestones found.</div>
      ) : (
        <div className={styles.milestoneList}>
          {filtered.map((m) => (
            <MilestoneCard
              key={m.id}
              milestone={m}
              isAdmin={isAdmin}
              onEdit={openEdit}
              onDelete={openDelete}
            />
          ))}
        </div>
      )}

      {/* Create Milestone Modal */}
      {modal === 'create' && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div
            className={styles.modal}
            role="dialog"
            aria-label="Create Milestone"
            onClick={(e) => e.stopPropagation()}
          >
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Create Milestone</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <form onSubmit={handleCreate} noValidate>
              <div className={styles.modalBody}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="ms-name">
                    Name <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="ms-name"
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
                  <label className={styles.formLabel} htmlFor="ms-desc">
                    Description <span className={styles.optional}>optional</span>
                  </label>
                  <textarea
                    id="ms-desc"
                    className={styles.formTextarea}
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    rows={3}
                  />
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="ms-due">
                    Due date <span className={styles.optional}>optional</span>
                  </label>
                  <input
                    id="ms-due"
                    className={styles.formInput}
                    type="date"
                    value={dueDate}
                    onChange={(e) => setDueDate(e.target.value)}
                  />
                </div>
              </div>
              <div className={styles.modalActions}>
                <button type="submit" className={styles.btnPrimary} disabled={isPending}>
                  {createMut.isPending ? 'Creating…' : 'Create Milestone'}
                </button>
                <button type="button" className={styles.btnGhostModal} onClick={closeModal}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Edit Milestone Modal */}
      {modal === 'edit' && editingMilestone && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div
            className={styles.modal}
            role="dialog"
            aria-label="Edit Milestone"
            onClick={(e) => e.stopPropagation()}
          >
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Edit Milestone</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <form onSubmit={handleEdit} noValidate>
              <div className={styles.modalBody}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-ms-name">
                    Name <span className={styles.required}>*</span>
                  </label>
                  <input
                    id="edit-ms-name"
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
                  <label className={styles.formLabel} htmlFor="edit-ms-desc">
                    Description <span className={styles.optional}>optional</span>
                  </label>
                  <textarea
                    id="edit-ms-desc"
                    className={styles.formTextarea}
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    rows={3}
                  />
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-ms-due">
                    Due date <span className={styles.optional}>optional</span>
                  </label>
                  <input
                    id="edit-ms-due"
                    className={styles.formInput}
                    type="date"
                    value={dueDate}
                    onChange={(e) => setDueDate(e.target.value)}
                  />
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel} htmlFor="edit-ms-status">
                    Status
                  </label>
                  <div className={styles.formSelectWrap}>
                    <select
                      id="edit-ms-status"
                      className={styles.formSelect}
                      value={status}
                      onChange={(e) => setStatus(e.target.value as MilestoneStatus)}
                    >
                      <option value="open">Open</option>
                      <option value="closed">Closed</option>
                    </select>
                  </div>
                </div>
              </div>
              <div className={styles.modalActions}>
                <button type="submit" className={styles.btnPrimary} disabled={isPending}>
                  {updateMut.isPending ? 'Saving…' : 'Save Changes'}
                </button>
                <button type="button" className={styles.btnGhostModal} onClick={closeModal}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Delete Milestone Modal */}
      {modal === 'delete' && editingMilestone && (
        <div className={styles.overlay} role="presentation" onClick={closeModal}>
          <div
            className={styles.modal}
            role="dialog"
            aria-label="Delete Milestone"
            onClick={(e) => e.stopPropagation()}
          >
            <div className={styles.modalHeader}>
              <h2 className={styles.modalTitle}>Delete Milestone</h2>
              <button className={styles.modalClose} onClick={closeModal} aria-label="Close">
                &times;
              </button>
            </div>
            {errors.server && <div className={styles.serverError}>{errors.server}</div>}
            <div className={styles.modalBody}>
              <p className={styles.deleteWarning}>
                Are you sure you want to delete <strong>{editingMilestone.name}</strong>? This
                action cannot be undone.
              </p>
              {editingMilestone.stats.total > 0 && (
                <p className={styles.deleteNote}>
                  This milestone has {editingMilestone.stats.total} assigned ticket
                  {editingMilestone.stats.total !== 1 ? 's' : ''} and cannot be deleted.
                </p>
              )}
            </div>
            <div className={styles.modalActions}>
              <button
                className={styles.btnDanger}
                onClick={handleDelete}
                disabled={isPending || editingMilestone.stats.total > 0}
              >
                {deleteMut.isPending ? 'Deleting…' : 'Delete'}
              </button>
              <button type="button" className={styles.btnGhostModal} onClick={closeModal}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function MilestoneStatusBadge({ status }: { status: MilestoneStatus }) {
  return (
    <span
      className={`${styles.statusBadge} ${status === 'open' ? styles.statusOpen : styles.statusClosed}`}
    >
      {status === 'open' ? 'Open' : 'Closed'}
    </span>
  );
}

interface MilestoneCardProps {
  milestone: Milestone;
  isAdmin: boolean;
  onEdit: (m: Milestone) => void;
  onDelete: (m: Milestone) => void;
}

function MilestoneCard({ milestone: m, isAdmin, onEdit, onDelete }: MilestoneCardProps) {
  const pct = donePercent(m);
  const segs = segmentWidths(m);
  const overdue = m.due_date ? isOverdue(m.due_date) && m.status === 'open' : false;

  return (
    <div className={styles.card}>
      <div className={styles.cardHeader}>
        <div className={styles.cardHeaderLeft}>
          <span className={styles.cardName}>{m.name}</span>
          <MilestoneStatusBadge status={m.status} />
        </div>
        <span className={`${styles.cardDue} ${overdue ? styles.cardDueOverdue : ''}`}>
          {m.due_date ? `Due ${formatDueDate(m.due_date)}` : '\u2014'}
        </span>
      </div>

      {m.description && <div className={styles.cardDesc}>{m.description}</div>}

      <div className={styles.progressWrap}>
        <div className={styles.progressBar}>
          {segs.done > 0 && (
            <div
              className={`${styles.progressSegment} ${styles.segmentDone}`}
              style={{ width: `${segs.done}%` }}
            />
          )}
          {segs.verify > 0 && (
            <div
              className={`${styles.progressSegment} ${styles.segmentVerify}`}
              style={{ width: `${segs.verify}%` }}
            />
          )}
          {segs.inProgress > 0 && (
            <div
              className={`${styles.progressSegment} ${styles.segmentInProgress}`}
              style={{ width: `${segs.inProgress}%` }}
            />
          )}
          {segs.new > 0 && (
            <div
              className={`${styles.progressSegment} ${styles.segmentNew}`}
              style={{ width: `${segs.new}%` }}
            />
          )}
        </div>
        <span className={styles.progressPct}>{pct}%</span>
      </div>

      <div className={styles.statsRow}>
        <span className={styles.stat}>
          <span className={styles.statDot} style={{ background: STATUS_COLORS.done }} />
          <span className={styles.statValue}>{m.stats.done}</span> Done
        </span>
        <span className={styles.stat}>
          <span className={styles.statDot} style={{ background: STATUS_COLORS.verify }} />
          <span className={styles.statValue}>{m.stats.verify}</span> Verify
        </span>
        <span className={styles.stat}>
          <span className={styles.statDot} style={{ background: STATUS_COLORS.in_progress }} />
          <span className={styles.statValue}>{m.stats.in_progress}</span> In Progress
        </span>
        <span className={styles.stat}>
          <span className={styles.statDot} style={{ background: STATUS_COLORS.new }} />
          <span className={styles.statValue}>{m.stats.new}</span> New
        </span>
      </div>

      <div className={styles.cardFooter}>
        <div className={styles.cardActions}>
          <Link
            to={`/tickets?q=milestone:${encodeURIComponent(m.name)}`}
            className={styles.btnGhost}
          >
            View Tickets
          </Link>
          {isAdmin && (
            <>
              <button className={styles.btnGhost} onClick={() => onEdit(m)}>
                Edit
              </button>
              <button
                className={`${styles.btnGhost} ${styles.btnGhostDanger}`}
                onClick={() => onDelete(m)}
              >
                Delete
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
