import type { Priority } from '../api/types';
import styles from './PriorityBadge.module.css';

const PRIORITY_CLASS: Record<Priority, string> = {
  P0: styles.p0,
  P1: styles.p1,
  P2: styles.p2,
  P3: styles.p3,
  P4: styles.p4,
  P5: styles.p5,
};

/** Signal-bar priority indicator with level label. */
export default function PriorityBadge({ priority }: { priority: Priority }) {
  return (
    <span className={`${styles.badge} ${PRIORITY_CLASS[priority]}`}>
      <span className={styles.bars}>
        <i className={styles.bar} />
        <i className={styles.bar} />
        <i className={styles.bar} />
        <i className={styles.bar} />
      </span>
      {priority}
    </span>
  );
}
