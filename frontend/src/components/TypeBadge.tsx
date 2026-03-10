import type { TicketType } from '../api/types';
import styles from './TypeBadge.module.css';

const TYPE_CLASS: Record<TicketType, string> = {
  bug: styles.bug,
  feature: styles.feature,
  task: styles.task,
  improvement: styles.improvement,
};

const TYPE_LABEL: Record<TicketType, string> = {
  bug: 'Bug',
  feature: 'Feature',
  task: 'Task',
  improvement: 'Improvement',
};

/** Colored type badge with dot indicator. */
export default function TypeBadge({ type }: { type: TicketType }) {
  return (
    <span className={`${styles.badge} ${TYPE_CLASS[type]}`}>
      {TYPE_LABEL[type]}
    </span>
  );
}
