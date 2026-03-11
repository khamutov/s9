import { useState, useMemo } from 'react';
import { Link } from 'react-router';
import { useMilestones } from './useMilestones';
import type { Milestone, MilestoneStatus } from '../../api/types';
import styles from './MilestoneListPage.module.css';

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

/** Milestone list page with progress cards, filter, and status stats. */
export default function MilestoneListPage() {
  const { data, isLoading, error } = useMilestones();
  const [filter, setFilter] = useState('');

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

  return (
    <div>
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.breadcrumb}>All milestones</div>
          <h1>
            Milestones {!isLoading && <span className={styles.pageCount}>{milestones.length}</span>}
          </h1>
        </div>
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
            <MilestoneCard key={m.id} milestone={m} />
          ))}
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

function MilestoneCard({ milestone: m }: { milestone: Milestone }) {
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
        </div>
      </div>
    </div>
  );
}
