import { usePageHeaderValue } from './usePageHeader';
import styles from './PageHeader.module.css';

/** Renders the breadcrumb, title, and optional subtitle for the current page. */
export default function PageHeader() {
  const config = usePageHeaderValue();
  if (!config) return null;

  const crumbs = ['S9', ...config.breadcrumb];

  return (
    <div className={styles.pageHeader}>
      <div>
        <div className={styles.breadcrumb}>
          {crumbs.map((crumb, i) => (
            <span key={i}>
              {i > 0 && ' / '}
              {crumb}
            </span>
          ))}
        </div>
        <h1 className={styles.title}>{config.title}</h1>
        {config.subtitle && <div className={styles.subtitle}>{config.subtitle}</div>}
      </div>
    </div>
  );
}
