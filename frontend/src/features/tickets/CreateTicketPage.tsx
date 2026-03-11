import { useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router';
import { MarkdownEditor } from '../../components/MarkdownEditor';
import { useCreateTicket } from './useCreateTicket';
import { useComponents } from '../components/useComponents';
import { useMilestones } from '../milestones/useMilestones';
import { useUsers } from '../admin/useUsers';
import type { TicketType, Priority, CreateTicketRequest } from '../../api/types';
import { ApiError } from '../../api/client';
import styles from './CreateTicketPage.module.css';

const TYPE_OPTIONS: { value: TicketType; label: string }[] = [
  { value: 'bug', label: 'Bug' },
  { value: 'feature', label: 'Feature' },
];

const PRIORITY_OPTIONS: { value: Priority; label: string }[] = [
  { value: 'P0', label: 'P0 Critical' },
  { value: 'P1', label: 'P1 High' },
  { value: 'P2', label: 'P2 Medium' },
  { value: 'P3', label: 'P3 Low' },
  { value: 'P4', label: 'P4 Minor' },
  { value: 'P5', label: 'P5 Trivial' },
];

interface FormErrors {
  title?: string;
  component_id?: string;
  owner_id?: string;
  server?: string;
}

/** Create ticket form with title, description, type, priority, component, owner, milestone. */
export default function CreateTicketPage() {
  const navigate = useNavigate();
  const mutation = useCreateTicket();
  const { data: componentsData } = useComponents();
  const { data: milestonesData } = useMilestones('open');
  const { data: usersData } = useUsers();

  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [type, setType] = useState<TicketType>('bug');
  const [priority, setPriority] = useState<Priority>('P2');
  const [componentId, setComponentId] = useState('');
  const [ownerId, setOwnerId] = useState('');
  const [milestoneId, setMilestoneId] = useState('');
  const [errors, setErrors] = useState<FormErrors>({});

  const components = componentsData?.items ?? [];
  const milestones = milestonesData?.items ?? [];
  const users = usersData?.items ?? [];

  const validate = (): boolean => {
    const next: FormErrors = {};
    if (!title.trim()) next.title = 'Title is required';
    if (!componentId) next.component_id = 'Component is required';
    if (!ownerId) next.owner_id = 'Owner is required';
    setErrors(next);
    return Object.keys(next).length === 0;
  };

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!validate()) return;

    const req: CreateTicketRequest = {
      title: title.trim(),
      type,
      priority,
      component_id: Number(componentId),
      owner_id: Number(ownerId),
    };

    if (description.trim()) {
      req.description = description.trim();
    }

    if (milestoneId) {
      req.milestones = [Number(milestoneId)];
    }

    mutation.mutate(req, {
      onSuccess: (ticket) => {
        navigate(`/tickets/${ticket.id}`);
      },
      onError: (err) => {
        if (err instanceof ApiError && err.details) {
          setErrors({ ...err.details });
        } else {
          setErrors({ server: 'Failed to create ticket. Please try again.' });
        }
      },
    });
  };

  const handleCancel = () => {
    navigate('/tickets');
  };

  return (
    <div>
      {errors.server && <div className={styles.serverError}>{errors.server}</div>}

      <form className={styles.formCard} onSubmit={handleSubmit} noValidate>
        {/* Basic Info */}
        <div className={styles.formSection}>
          <div className={styles.formSectionTitle}>Basic Info</div>

          <div className={styles.formGroup}>
            <label className={styles.formLabel} htmlFor="ticket-title">
              Title <span className={styles.required}>*</span>
            </label>
            <input
              id="ticket-title"
              className={`${styles.formInputTitle} ${errors.title ? styles.formInputError : ''}`}
              type="text"
              placeholder="Ticket title..."
              value={title}
              onChange={(e) => {
                setTitle(e.target.value);
                if (errors.title && e.target.value.trim()) {
                  setErrors((prev) => ({ ...prev, title: undefined }));
                }
              }}
              autoFocus
            />
            {errors.title && <div className={styles.formError}>{errors.title}</div>}
          </div>

          <div className={`${styles.formGroup} ${styles.descriptionWrap}`}>
            <label className={styles.formLabel}>
              Description <span className={styles.optional}>optional</span>
            </label>
            <MarkdownEditor
              value={description}
              onChange={setDescription}
              placeholder="Describe the issue..."
              minHeight={160}
              disabled={mutation.isPending}
            />
          </div>
        </div>

        {/* Metadata */}
        <div className={styles.formSection}>
          <div className={styles.formSectionTitle}>Metadata</div>

          <div className={styles.metaGrid}>
            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="ticket-type">
                Type
              </label>
              <div className={styles.formSelectWrap}>
                <select
                  id="ticket-type"
                  className={styles.formSelect}
                  value={type}
                  onChange={(e) => setType(e.target.value as TicketType)}
                >
                  {TYPE_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="ticket-priority">
                Priority
              </label>
              <div className={styles.formSelectWrap}>
                <select
                  id="ticket-priority"
                  className={styles.formSelect}
                  value={priority}
                  onChange={(e) => setPriority(e.target.value as Priority)}
                >
                  {PRIORITY_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="ticket-component">
                Component <span className={styles.required}>*</span>
              </label>
              <div className={styles.formSelectWrap}>
                <select
                  id="ticket-component"
                  className={`${styles.formSelect} ${errors.component_id ? styles.formInputError : ''}`}
                  value={componentId}
                  onChange={(e) => {
                    setComponentId(e.target.value);
                    if (errors.component_id && e.target.value) {
                      setErrors((prev) => ({ ...prev, component_id: undefined }));
                    }
                  }}
                >
                  <option value="">Select component...</option>
                  {components.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.path}
                    </option>
                  ))}
                </select>
              </div>
              {errors.component_id && <div className={styles.formError}>{errors.component_id}</div>}
            </div>

            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="ticket-owner">
                Owner <span className={styles.required}>*</span>
              </label>
              <div className={styles.formSelectWrap}>
                <select
                  id="ticket-owner"
                  className={`${styles.formSelect} ${errors.owner_id ? styles.formInputError : ''}`}
                  value={ownerId}
                  onChange={(e) => {
                    setOwnerId(e.target.value);
                    if (errors.owner_id && e.target.value) {
                      setErrors((prev) => ({ ...prev, owner_id: undefined }));
                    }
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

            <div className={styles.formGroup}>
              <label className={styles.formLabel} htmlFor="ticket-milestone">
                Milestone <span className={styles.optional}>optional</span>
              </label>
              <div className={styles.formSelectWrap}>
                <select
                  id="ticket-milestone"
                  className={styles.formSelect}
                  value={milestoneId}
                  onChange={(e) => setMilestoneId(e.target.value)}
                >
                  <option value="">No milestone</option>
                  {milestones.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className={styles.formActions}>
          <button type="submit" className={styles.btnPrimary} disabled={mutation.isPending}>
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
            {mutation.isPending ? 'Creating…' : 'Create Ticket'}
          </button>
          <button type="button" className={styles.cancelBtn} onClick={handleCancel}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
