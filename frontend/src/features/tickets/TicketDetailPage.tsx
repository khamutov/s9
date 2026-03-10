import { useParams, Link } from 'react-router';
import { usePageHeader } from '../../components/layout/usePageHeader';
import StatusBadge from '../../components/StatusBadge';
import PriorityBadge from '../../components/PriorityBadge';
import TypeBadge from '../../components/TypeBadge';
import UserPill from '../../components/UserPill';
import { useTicket } from './useTicket';
import { useComments } from './useComments';
import type { Comment, Ticket } from '../../api/types';
import styles from './TicketDetailPage.module.css';

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

/** Metadata sidebar panel displaying ticket details. */
function MetadataPanel({ ticket }: { ticket: Ticket }) {
  return (
    <div className={styles.metaPanel}>
      <div className={styles.metaPanelHeader}>Details</div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Status</span>
        <span className={styles.metaFieldValue}>
          <StatusBadge status={ticket.status} />
        </span>
      </div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Priority</span>
        <span className={styles.metaFieldValue}>
          <PriorityBadge priority={ticket.priority} />
        </span>
      </div>

      <div className={styles.metaField}>
        <span className={styles.metaFieldLabel}>Type</span>
        <span className={styles.metaFieldValue}>
          <TypeBadge type={ticket.type} />
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

      {ticket.estimation_display && (
        <div className={styles.metaField}>
          <span className={styles.metaFieldLabel}>Estimate</span>
          <span className={styles.metaFieldValue}>
            <span className={styles.estimateValue}>
              {ticket.estimation_display}
            </span>
          </span>
        </div>
      )}

      <div className={styles.metaDates}>
        <div className={styles.metaDate}>
          <span>Created</span>
          <span className={styles.metaDateValue}>
            {formatDate(ticket.created_at)}
          </span>
        </div>
        <div className={styles.metaDate}>
          <span>Updated</span>
          <span className={styles.metaDateValue}>
            {formatDate(ticket.updated_at)}
          </span>
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
          <span className={styles.commentTime}>
            {formatRelativeTime(comment.created_at)}
          </span>
        </div>
        <div className={styles.prose}>
          {comment.body.split('\n').map((line, i) => (
            <p key={i}>{line || '\u00A0'}</p>
          ))}
        </div>
      </div>
    </div>
  );
}

/** Ticket detail view with metadata sidebar, description, and comment thread. */
export default function TicketDetailPage() {
  const { id } = useParams<{ id: string }>();
  const ticketId = Number(id);
  const { data: ticket, isLoading, error } = useTicket(ticketId);
  const {
    data: commentsData,
    isLoading: commentsLoading,
  } = useComments(ticketId);

  const displaySlug = ticket?.slug ?? `#${id}`;
  usePageHeader({
    title: ticket?.title ?? `Ticket ${displaySlug}`,
    breadcrumb: ['Tickets'],
  });

  if (isLoading) {
    return <div className={styles.loading}>Loading ticket…</div>;
  }

  if (error || !ticket) {
    return (
      <div className={styles.error}>
        Failed to load ticket. Please try again.
      </div>
    );
  }

  const comments = commentsData?.items ?? [];
  // Comment #0 is the description; remaining are activity
  const description = comments.find((c) => c.number === 0);
  const activityComments = comments.filter((c) => c.number > 0);

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
                <div className={styles.prose}>
                  {description.body.split('\n').map((line, i) => (
                    <p key={i}>{line || '\u00A0'}</p>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Activity section */}
          <div className={styles.activitySection}>
            <div className={styles.activityHeader}>
              <h2>Activity</h2>
              {!commentsLoading && (
                <span className={styles.activityCount}>
                  {activityComments.length}
                </span>
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
        <MetadataPanel ticket={ticket} />
      </div>
    </div>
  );
}
