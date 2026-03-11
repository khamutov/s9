import type { TicketType } from '../api/types';
import styles from './TypeBadge.module.css';

const TYPE_CLASS: Record<TicketType, string> = {
  bug: styles.bug,
  feature: styles.feature,
};

const TYPE_LABEL: Record<TicketType, string> = {
  bug: 'Bug',
  feature: 'Feature',
};

/** Colored type badge with dot indicator. */
export default function TypeBadge({ type }: { type: TicketType }) {
  return <span className={`${styles.badge} ${TYPE_CLASS[type]}`}>{TYPE_LABEL[type]}</span>;
}
