import { useCallback, useMemo, useRef, useState } from 'react';
import { useParams, Link } from 'react-router';
import StatusBadge from '../../components/StatusBadge';
import PriorityBadge from '../../components/PriorityBadge';
import TypeBadge from '../../components/TypeBadge';
import UserPill from '../../components/UserPill';
import InlineSelect, { type SelectOption } from '../../components/InlineSelect';
import InlineText from '../../components/InlineText';
import MarkdownRenderer from '../../components/MarkdownRenderer';
import { MarkdownEditor } from '../../components/MarkdownEditor';
import AttachmentList from '../../components/AttachmentList';
import { useAuth } from '../auth/useAuth';
import { useTicket } from './useTicket';
import { useComments } from './useComments';
import { useUpdateTicket } from './useUpdateTicket';
import { useCreateComment } from './useCreateComment';
import { useEditComment, useDeleteComment } from './useEditComment';
import { useCompactUsers } from './useCompactUsers';
import type {
  Comment,
  CompactUser,
  Ticket,
  TicketStatus,
  Priority,
  TicketType,
} from '../../api/types';
import styles from './TicketDetailPage.module.css';

const STATUS_OPTIONS: SelectOption<TicketStatus>[] = [
  { value: 'new', label: 'New' },
  { value: 'in_progress', label: 'In Progress' },
  { value: 'verify', label: 'Verify' },
  { value: 'done', label: 'Done' },
];

const PRIORITY_OPTIONS: SelectOption<Priority>[] = [
  { value: 'P0', label: 'P0' },
  { value: 'P1', label: 'P1' },
  { value: 'P2', label: 'P2' },
  { value: 'P3', label: 'P3' },
  { value: 'P4', label: 'P4' },
  { value: 'P5', label: 'P5' },
];

const TYPE_OPTIONS: SelectOption<TicketType>[] = [
  { value: 'bug', label: 'Bug' },
  { value: 'feature', label: 'Feature' },
  { value: 'task', label: 'Task' },
  { value: 'improvement', label: 'Improvement' },
];

/** Formats a UTC ISO date string as a human-readable relative time. */
function formatRelativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffMs = now - then;
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 1) return 'just now';
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

/** Formats a UTC ISO date as "Mar 6, 2026". */
function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

/** Metadata sidebar panel with inline-editable fields. */
function MetadataPanel({
  ticket,
  onUpdate,
  users,
  onOwnerChange,
}: {
  ticket: Ticket;
  onUpdate: (field: string, value: unknown) => void;
  users: CompactUser[];
  onOwnerChange: (userId: number) => void;
}) {
  const ownerOptions: SelectOption<string>[] = useMemo(
    () => users.map((u) => ({ value: String(u.id), label: u.display_name })),
    [users],
  );

  const usersById = useMemo(() => {
    const map = new Map<number, CompactUser>();
    for (const u of users) map.set(u.id, u);
    return map;
  }, [users]);
  return (
    <div className={styles.metaPanel}>
      <div className={styles.metaPanelHeader}>Details</div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Status</span>
        <span className={styles.metaFieldValue}>
          <InlineSelect
            value={ticket.status}
            options={STATUS_OPTIONS}
            onChange={(v) => onUpdate('status', v)}
            renderValue={(v) => <StatusBadge status={v} />}
            renderOption={(v) => <StatusBadge status={v} />}
            aria-label="Status"
          />
        </span>
      </div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Priority</span>
        <span className={styles.metaFieldValue}>
          <InlineSelect
            value={ticket.priority}
            options={PRIORITY_OPTIONS}
            onChange={(v) => onUpdate('priority', v)}
            renderValue={(v) => <PriorityBadge priority={v} />}
            renderOption={(v) => <PriorityBadge priority={v} />}
            aria-label="Priority"
          />
        </span>
      </div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Type</span>
        <span className={styles.metaFieldValue}>
          <InlineSelect
            value={ticket.type}
            options={TYPE_OPTIONS}
            onChange={(v) => onUpdate('type', v)}
            renderValue={(v) => <TypeBadge type={v} />}
            renderOption={(v) => <TypeBadge type={v} />}
            aria-label="Type"
          />
        </span>
      </div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Owner</span>
        <span className={styles.metaFieldValue}>
          <InlineSelect
            value={String(ticket.owner.id)}
            options={ownerOptions}
            onChange={(v) => onOwnerChange(Number(v))}
            renderValue={() => <UserPill user={ticket.owner} small />}
            renderOption={(v) => {
              const u = usersById.get(Number(v));
              return u ? <UserPill user={u} small /> : v;
            }}
            aria-label="Owner"
          />
        </span>
      </div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Reporter</span>
        <span className={styles.metaFieldValue}>
          <UserPill user={ticket.created_by} small />
        </span>
      </div>

      {ticket.cc.length > 0 && (
        <div className={styles.metaField}>
          <span className={styles.metaFieldLabel}>CC</span>
          <span className={styles.metaFieldValue}>
            <div className={styles.ccList}>
              {ticket.cc.map((user) => (
                <UserPill key={user.id} user={user} small />
              ))}
            </div>
          </span>
        </div>
      )}

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Component</span>
        <span className={styles.metaFieldValue}>
          <span className={styles.componentPath}>{ticket.component.path}</span>
        </span>
      </div>

      {ticket.milestones.length > 0 && (
        <div className={styles.metaField}>
          <span className={styles.metaFieldLabel}>Milestone</span>
          <span className={styles.metaFieldValue}>
            {ticket.milestones.map((m) => (
              <span key={m.id} className={styles.milestoneChip}>
                {m.name}
              </span>
            ))}
          </span>
        </div>
      )}

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Estimate</span>
        <span className={styles.metaFieldValue}>
          <InlineText
            value={ticket.estimation_display ?? ''}
            onSave={(v) => onUpdate('estimation', v || null)}
            aria-label="Estimate"
            placeholder="None"
          >
            {ticket.estimation_display ? (
              <span className={styles.estimateValue}>{ticket.estimation_display}</span>
            ) : undefined}
          </InlineText>
        </span>
      </div>

      <div className={styles.metaDates}>
        <div className={styles.metaDate}>
          <span>Created</span>
          <span className={styles.metaDateValue}>{formatDate(ticket.created_at)}</span>
        </div>
        <div className={styles.metaDate}>
          <span>Updated</span>
          <span className={styles.metaDateValue}>{formatDate(ticket.updated_at)}</span>
        </div>
      </div>
    </div>
  );
}

/** Single comment card in the activity thread with edit/delete support. */
function CommentCard({
  comment,
  ticketId,
  currentUserId,
  isAdmin,
}: {
  comment: Comment;
  ticketId: number;
  currentUserId: number | null;
  isAdmin: boolean;
}) {
  const [editing, setEditing] = useState(false);
  const [editBody, setEditBody] = useState(comment.body);
  const editMutation = useEditComment(ticketId);
  const deleteMutation = useDeleteComment(ticketId);

  const canEdit = currentUserId === comment.author.id || isAdmin;
  const canDelete = isAdmin && comment.number > 0;

  const handleSaveEdit = () => {
    const trimmed = editBody.trim();
    if (!trimmed || trimmed === comment.body) {
      setEditing(false);
      setEditBody(comment.body);
      return;
    }
    editMutation.mutate(
      { commentNum: comment.number, req: { body: trimmed } },
      {
        onSuccess: () => setEditing(false),
      },
    );
  };

  const handleCancelEdit = () => {
    setEditing(false);
    setEditBody(comment.body);
  };

  const handleDelete = () => {
    deleteMutation.mutate(comment.number);
  };

  return (
    <div className={styles.comment} id={`comment-${comment.number}`}>
      <div className={styles.commentCard}>
        <div className={styles.commentHeader}>
          <UserPill user={comment.author} small />
          <a className={styles.commentAnchor} href={`#comment-${comment.number}`}>
            #{comment.number}
          </a>
          {comment.edit_count > 0 && <span className={styles.editedTag}>edited</span>}
          <span className={styles.commentTime}>{formatRelativeTime(comment.created_at)}</span>
          {(canEdit || canDelete) && !editing && (
            <div className={styles.commentActions}>
              {canEdit && (
                <button
                  className={styles.commentActionBtn}
                  onClick={() => setEditing(true)}
                  aria-label={`Edit comment #${comment.number}`}
                >
                  Edit
                </button>
              )}
              {canDelete && (
                <button
                  className={styles.commentActionBtn}
                  onClick={handleDelete}
                  disabled={deleteMutation.isPending}
                  aria-label={`Delete comment #${comment.number}`}
                >
                  {deleteMutation.isPending ? 'Deleting…' : 'Delete'}
                </button>
              )}
            </div>
          )}
        </div>
        {editing ? (
          <div className={styles.editForm}>
            <MarkdownEditor
              value={editBody}
              onChange={setEditBody}
              placeholder="Edit comment…"
              minHeight={80}
              disabled={editMutation.isPending}
            />
            <div className={styles.editFormActions}>
              <button
                className={styles.cancelBtn}
                onClick={handleCancelEdit}
                disabled={editMutation.isPending}
              >
                Cancel
              </button>
              <button
                className={styles.submitBtn}
                onClick={handleSaveEdit}
                disabled={editMutation.isPending || !editBody.trim()}
              >
                {editMutation.isPending ? 'Saving…' : 'Save'}
              </button>
            </div>
            {editMutation.isError && (
              <p className={styles.formError}>Failed to save edit. Please try again.</p>
            )}
          </div>
        ) : (
          <>
            <MarkdownRenderer>{comment.body}</MarkdownRenderer>
            <AttachmentList attachments={comment.attachments} />
          </>
        )}
      </div>
    </div>
  );
}

/** Form for adding a new comment to the ticket. */
function CommentForm({ ticketId }: { ticketId: number }) {
  const [body, setBody] = useState('');
  const attachmentIdsRef = useRef<number[]>([]);
  const mutation = useCreateComment(ticketId);

  const handleAttachmentUploaded = useCallback((id: number) => {
    attachmentIdsRef.current.push(id);
  }, []);

  const handleSubmit = () => {
    const trimmed = body.trim();
    if (!trimmed) return;
    const ids = attachmentIdsRef.current;
    const req = ids.length > 0 ? { body: trimmed, attachment_ids: ids } : { body: trimmed };
    mutation.mutate(req, {
      onSuccess: () => {
        setBody('');
        attachmentIdsRef.current = [];
      },
    });
  };

  return (
    <div className={styles.commentForm}>
      <div className={styles.commentFormLabel}>Add comment</div>
      <MarkdownEditor
        value={body}
        onChange={setBody}
        placeholder="Write a comment… Use @mentions and #references"
        minHeight={100}
        disabled={mutation.isPending}
        onAttachmentUploaded={handleAttachmentUploaded}
      />
      <div className={styles.commentFormFooter}>
        {mutation.isError && (
          <p className={styles.formError}>Failed to post comment. Please try again.</p>
        )}
        <button
          className={styles.submitBtn}
          onClick={handleSubmit}
          disabled={mutation.isPending || !body.trim()}
        >
          {mutation.isPending ? 'Posting…' : 'Comment'}
        </button>
      </div>
    </div>
  );
}

/** Editable description card for comment #0. */
function DescriptionCard({
  description,
  ticketId,
  canEdit,
}: {
  description: Comment;
  ticketId: number;
  canEdit: boolean;
}) {
  const [editing, setEditing] = useState(false);
  const [editBody, setEditBody] = useState(description.body);
  const editMutation = useEditComment(ticketId);

  const handleSave = () => {
    const trimmed = editBody.trim();
    if (!trimmed || trimmed === description.body) {
      setEditing(false);
      setEditBody(description.body);
      return;
    }
    editMutation.mutate(
      { commentNum: 0, req: { body: trimmed } },
      { onSuccess: () => setEditing(false) },
    );
  };

  const handleCancel = () => {
    setEditing(false);
    setEditBody(description.body);
  };

  return (
    <div className={styles.descriptionSection}>
      <div className={styles.descriptionCard}>
        <div className={styles.descriptionHeader}>
          <UserPill user={description.author} small />
          <span className={styles.commentTime}>
            opened {formatRelativeTime(description.created_at)}
          </span>
          {description.edit_count > 0 && <span className={styles.editedTag}>edited</span>}
          {canEdit && !editing && (
            <button
              className={styles.commentActionBtn}
              onClick={() => {
                setEditBody(description.body);
                setEditing(true);
              }}
              aria-label="Edit description"
            >
              Edit
            </button>
          )}
        </div>
        {editing ? (
          <div className={styles.editForm}>
            <MarkdownEditor
              value={editBody}
              onChange={setEditBody}
              placeholder="Edit description…"
              minHeight={120}
              disabled={editMutation.isPending}
            />
            <div className={styles.editFormActions}>
              <button
                className={styles.cancelBtn}
                onClick={handleCancel}
                disabled={editMutation.isPending}
              >
                Cancel
              </button>
              <button
                className={styles.submitBtn}
                onClick={handleSave}
                disabled={editMutation.isPending || !editBody.trim()}
              >
                {editMutation.isPending ? 'Saving…' : 'Save'}
              </button>
            </div>
            {editMutation.isError && (
              <p className={styles.formError}>Failed to save. Please try again.</p>
            )}
          </div>
        ) : (
          <>
            <MarkdownRenderer>{description.body}</MarkdownRenderer>
            <AttachmentList attachments={description.attachments} />
          </>
        )}
      </div>
    </div>
  );
}

/** Ticket detail view with metadata sidebar, description, and comment thread. */
export default function TicketDetailPage() {
  const { id } = useParams<{ id: string }>();
  const ticketId = Number(id);
  const { user } = useAuth();
  const { data: ticket, isLoading, error } = useTicket(ticketId);
  const { data: commentsData, isLoading: commentsLoading } = useComments(ticketId);
  const mutation = useUpdateTicket(ticketId);
  const { data: usersData } = useCompactUsers();
  const isAdmin = user?.role === 'admin';

  const displaySlug = ticket?.slug ?? `#${id}`;

  if (isLoading) {
    return <div className={styles.loading}>Loading ticket…</div>;
  }

  if (error || !ticket) {
    return <div className={styles.error}>Failed to load ticket. Please try again.</div>;
  }

  const comments = commentsData?.items ?? [];
  // Comment #0 is the description; remaining are activity
  const description = comments.find((c) => c.number === 0);
  const activityComments = comments.filter((c) => c.number > 0);

  const handleUpdate = (field: string, value: unknown) => {
    mutation.mutate({ [field]: value });
  };

  return (
    <div>
      {/* Page header */}
      <div className={styles.header}>
        <div className={styles.breadcrumb}>
          <Link to="/tickets">Tickets</Link>
          <span className={styles.breadcrumbSep}>/</span>
          <span>{displaySlug}</span>
        </div>
        <div className={styles.headerTop}>
          <h1>
            <InlineText
              value={ticket.title}
              onSave={(v) => {
                if (v.trim()) handleUpdate('title', v.trim());
              }}
              aria-label="Title"
            />
          </h1>
          <span className={styles.ticketId}>{displaySlug}</span>
        </div>
        <div className={styles.badges}>
          <TypeBadge type={ticket.type} />
          <StatusBadge status={ticket.status} />
          <PriorityBadge priority={ticket.priority} />
        </div>
      </div>

      {/* Two-column layout */}
      <div className={styles.content}>
        {/* Left column: Description + Comments */}
        <div>
          {/* Description (Comment #0) */}
          {description && (
            <DescriptionCard
              description={description}
              ticketId={ticketId}
              canEdit={user?.id === description.author.id || isAdmin}
            />
          )}

          {/* Activity section */}
          <div className={styles.activitySection}>
            <div className={styles.activityHeader}>
              <h2>Activity</h2>
              {!commentsLoading && (
                <span className={styles.activityCount}>{activityComments.length}</span>
              )}
            </div>

            {commentsLoading ? (
              <div className={styles.loading}>Loading comments…</div>
            ) : activityComments.length === 0 ? (
              <p className={styles.emptyComments}>No comments yet.</p>
            ) : (
              <div className={styles.commentThread}>
                {activityComments.map((comment) => (
                  <CommentCard
                    key={comment.number}
                    comment={comment}
                    ticketId={ticketId}
                    currentUserId={user?.id ?? null}
                    isAdmin={isAdmin}
                  />
                ))}
              </div>
            )}

            <CommentForm ticketId={ticketId} />
          </div>
        </div>

        {/* Right column: Metadata */}
        <MetadataPanel
          ticket={ticket}
          onUpdate={handleUpdate}
          users={usersData?.items ?? []}
          onOwnerChange={(userId) => handleUpdate('owner_id', userId)}
        />
      </div>
    </div>
  );
}
