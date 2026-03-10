import type { TicketStatus } from '../api/types';
import styles from './StatusBadge.module.css';

const STATUS_CLASS: Record<TicketStatus, string> = {
  new: styles.new,
  in_progress: styles.inProgress,
  verify: styles.verify,
  done: styles.done,
};

const STATUS_LABEL: Record<TicketStatus, string> = {
  new: 'New',
  in_progress: 'In Progress',
  verify: 'Verify',
  done: 'Done',
};

/** Colored status badge with dot indicator. */
export default function StatusBadge({ status }: { status: TicketStatus }) {
  return <span className={`${styles.badge} ${STATUS_CLASS[status]}`}>{STATUS_LABEL[status]}</span>;
}
