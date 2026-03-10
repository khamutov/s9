import { useParams, Link } from 'react-router';
import { usePageHeader } from '../../components/layout/usePageHeader';
import StatusBadge from '../../components/StatusBadge';
import PriorityBadge from '../../components/PriorityBadge';
import TypeBadge from '../../components/TypeBadge';
import UserPill from '../../components/UserPill';
import InlineSelect, { type SelectOption } from '../../components/InlineSelect';
import InlineText from '../../components/InlineText';
import MarkdownRenderer from '../../components/MarkdownRenderer';
import { useTicket } from './useTicket';
import { useComments } from './useComments';
import { useUpdateTicket } from './useUpdateTicket';
import type { Comment, Ticket, TicketStatus, Priority, TicketType } from '../../api/types';
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
}: {
  ticket: Ticket;
  onUpdate: (field: string, value: unknown) => void;
}) {
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
          <UserPill user={ticket.owner} small />
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

/** Single comment card in the activity thread. */
function CommentCard({ comment }: { comment: Comment }) {
  return (
    <div className={styles.comment}>
      <div className={styles.commentCard}>
        <div className={styles.commentHeader}>
          <UserPill user={comment.author} small />
          <a className={styles.commentAnchor} href={`#comment-${comment.number}`}>
            #{comment.number}
          </a>
          <span className={styles.commentTime}>{formatRelativeTime(comment.created_at)}</span>
        </div>
        <MarkdownRenderer>{comment.body}</MarkdownRenderer>
      </div>
    </div>
  );
}

/** Ticket detail view with metadata sidebar, description, and comment thread. */
export default function TicketDetailPage() {
  const { id } = useParams<{ id: string }>();
  const ticketId = Number(id);
  const { data: ticket, isLoading, error } = useTicket(ticketId);
  const { data: commentsData, isLoading: commentsLoading } = useComments(ticketId);
  const mutation = useUpdateTicket(ticketId);

  const displaySlug = ticket?.slug ?? `#${id}`;
  usePageHeader({
    title: ticket?.title ?? `Ticket ${displaySlug}`,
    breadcrumb: ['Tickets'],
  });

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
          <h1>{ticket.title}</h1>
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
            <div className={styles.descriptionSection}>
              <div className={styles.descriptionCard}>
                <div className={styles.descriptionHeader}>
                  <UserPill user={description.author} small />
                  <span className={styles.commentTime}>
                    opened {formatRelativeTime(description.created_at)}
                  </span>
                </div>
                <MarkdownRenderer>{description.body}</MarkdownRenderer>
              </div>
            </div>
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
                  <CommentCard key={comment.number} comment={comment} />
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Right column: Metadata */}
        <MetadataPanel ticket={ticket} onUpdate={handleUpdate} />
      </div>
    </div>
  );
}
