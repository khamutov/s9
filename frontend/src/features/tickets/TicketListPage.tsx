import { useState, useEffect } from 'react';
import { useNavigate, Link, useSearchParams } from 'react-router';
import { usePageHeader } from '../../components/layout/usePageHeader';
import StatusBadge from '../../components/StatusBadge';
import PriorityBadge from '../../components/PriorityBadge';
import UserPill from '../../components/UserPill';
import FilterBar from '../../components/FilterBar';
import { useTickets } from './useTickets';
import { isOffsetPage } from '../../api/types';
import type { Ticket, TicketStatus } from '../../api/types';
import styles from './TicketListPage.module.css';

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

/** Counts tickets per status for the summary bar. */
function countStatuses(tickets: Ticket[]): Record<TicketStatus, number> {
  const counts: Record<TicketStatus, number> = {
    new: 0,
    in_progress: 0,
    verify: 0,
    done: 0,
  };
  for (const t of tickets) {
    counts[t.status]++;
  }
  return counts;
}

const STATUS_COLORS: Record<TicketStatus, string> = {
  new: 'var(--status-new)',
  in_progress: 'var(--status-progress)',
  verify: 'var(--status-verify)',
  done: 'var(--status-done)',
};

const STATUS_LABELS: Record<TicketStatus, string> = {
  new: 'New',
  in_progress: 'In Progress',
  verify: 'Verify',
  done: 'Done',
};

/** Paginated ticket list with table, status summary, and navigation. */
/** Debounce delay in ms before sending filter query to the API. */
const DEBOUNCE_MS = 300;

export default function TicketListPage() {
  usePageHeader({ title: 'Tickets', breadcrumb: [] });
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  // Filter input is controlled; debounced value is sent to API
  const [filterText, setFilterText] = useState(searchParams.get('q') ?? '');
  const [debouncedQ, setDebouncedQ] = useState(filterText);

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQ(filterText);
      // Sync query param with URL for shareability
      if (filterText) {
        setSearchParams({ q: filterText }, { replace: true });
      } else {
        setSearchParams({}, { replace: true });
      }
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [filterText, setSearchParams]);

  const { data, isLoading, error } = useTickets(
    debouncedQ ? { q: debouncedQ } : {},
  );

  const tickets = data?.items ?? [];
  const totalCount = data ? (isOffsetPage(data) ? data.total : tickets.length) : 0;
  const counts = countStatuses(tickets);

  return (
    <div>
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.breadcrumb}>All tickets</div>
          <h1>
            Tickets{' '}
            {!isLoading && <span className={styles.pageCount}>{totalCount}</span>}
          </h1>
        </div>
        <Link to="/tickets/new" className={styles.createBtn}>
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
          Create Ticket
        </Link>
      </div>

      <FilterBar value={filterText} onChange={setFilterText} />

      <div className={styles.tableWrap}>
        {isLoading ? (
          <div className={styles.emptyState}>Loading tickets…</div>
        ) : error ? (
          <div className={styles.errorState}>
            Failed to load tickets. Please try again.
          </div>
        ) : tickets.length === 0 ? (
          <div className={styles.emptyState}>No tickets found.</div>
        ) : (
          <>
            <table className={styles.table}>
              <thead>
                <tr>
                  <th className={styles.colId}>ID</th>
                  <th className={styles.colTitle}>Title</th>
                  <th className={styles.colStatus}>Status</th>
                  <th className={styles.colPriority}>Pri</th>
                  <th className={styles.colOwner}>Owner</th>
                  <th className={styles.colComponent}>Component</th>
                  <th className={styles.colUpdated}>Updated</th>
                </tr>
              </thead>
              <tbody>
                {tickets.map((ticket) => (
                  <tr
                    key={ticket.id}
                    className={styles.row}
                    onClick={() => navigate(`/tickets/${ticket.id}`)}
                    role="link"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') navigate(`/tickets/${ticket.id}`);
                    }}
                  >
                    <td className={styles.colId}>
                      {ticket.slug ?? `#${ticket.id}`}
                    </td>
                    <td className={styles.colTitle}>{ticket.title}</td>
                    <td className={styles.colStatus}>
                      <StatusBadge status={ticket.status} />
                    </td>
                    <td className={styles.colPriority}>
                      <PriorityBadge priority={ticket.priority} />
                    </td>
                    <td className={styles.colOwner}>
                      <UserPill user={ticket.owner} small />
                    </td>
                    <td
                      className={`${styles.colComponent} ${styles.textSecondary}`}
                    >
                      {ticket.component.name}
                    </td>
                    <td className={styles.colUpdated}>
                      {formatRelativeTime(ticket.updated_at)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className={styles.summaryBar}>
              {(
                ['new', 'in_progress', 'verify', 'done'] as TicketStatus[]
              ).map((status) => (
                <span key={status} className={styles.stat}>
                  <span
                    className={styles.statDot}
                    style={{ background: STATUS_COLORS[status] }}
                  />
                  <span className={styles.statValue}>{counts[status]}</span>{' '}
                  {STATUS_LABELS[status]}
                </span>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
