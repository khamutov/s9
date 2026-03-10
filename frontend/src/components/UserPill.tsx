import type { CompactUser } from '../api/types';
import styles from './UserPill.module.css';

const AVATAR_COLORS = ['#a78bfa', '#2dd4bf', '#e8b43a', '#fb7185', '#60a5fa', '#34d399'];

function getInitials(name: string): string {
  const parts = name.split(/\s+/);
  if (parts.length >= 2) return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
  return name.slice(0, 2).toUpperCase();
}

function getAvatarColor(id: number): string {
  return AVATAR_COLORS[id % AVATAR_COLORS.length];
}

/** User display with colored avatar initials. */
export default function UserPill({
  user,
  small,
}: {
  user: CompactUser;
  small?: boolean;
}) {
  return (
    <span className={`${styles.pill} ${small ? styles.small : ''}`}>
      <span
        className={styles.avatar}
        style={{ background: getAvatarColor(user.id) }}
      >
        {getInitials(user.display_name)}
      </span>
      {user.display_name}
    </span>
  );
}
