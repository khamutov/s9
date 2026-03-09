import { NavLink, useLocation } from 'react-router';
import { NAV_SECTIONS } from './navConfig';
import styles from './Sidebar.module.css';

/** Main sidebar navigation. */
export default function Sidebar() {
  const location = useLocation();

  return (
    <aside className={styles.sidebar}>
      {/* Logo + notification bell */}
      <div className={styles.header}>
        <span className={styles.logo}>
          S9<span className={styles.logoDot} />
        </span>
        <button className={styles.bell} title="Notifications">
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M6 13.5a2 2 0 0 0 4 0" />
            <path d="M12.5 6.5a4.5 4.5 0 1 0-9 0c0 2.5-.5 4-1.5 5.5h12c-1-.5-1.5-3-1.5-5.5z" />
          </svg>
          <span className={styles.bellDot} />
        </button>
      </div>

      {/* Navigation sections */}
      <nav className={styles.nav}>
        {NAV_SECTIONS.map((section) => (
          <div key={section.label}>
            <div className={styles.sectionLabel}>{section.label}</div>
            {section.items.map((item) => {
              const hasQuery = item.path.includes('?');
              return (
                <NavLink
                  key={item.path}
                  to={item.path}
                  end={item.end}
                  className={({ isActive }) => {
                    // For items with query strings, match path+search exactly
                    const active = hasQuery
                      ? location.pathname + location.search === item.path
                      : isActive;
                    return `${styles.navItem}${active ? ` ${styles.active}` : ''}`;
                  }}
                >
                  {item.icon}
                  <span>{item.label}</span>
                </NavLink>
              );
            })}
          </div>
        ))}
      </nav>

      {/* User pill (hardcoded until AuthContext) */}
      <div className={styles.footer}>
        <div className={styles.userPill}>
          <span className={styles.userAvatar}>AK</span>
          <span>Alex Kim</span>
        </div>
      </div>
    </aside>
  );
}
