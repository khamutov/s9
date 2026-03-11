import { useAuth } from '../auth/useAuth';
import styles from './SystemSettings.module.css';

/** System settings and configuration panel. Read-only for now. */
export default function SystemSettings() {
  const { user } = useAuth();

  if (user?.role !== 'admin') {
    return (
      <div className={styles.accessDenied}>
        You need administrator privileges to access this page.
      </div>
    );
  }

  return (
    <div>
      <div className={styles.header}>
        <div className={styles.breadcrumb}>Administration</div>
        <h1>System Settings</h1>
      </div>

      <div className={styles.card}>
        <div className={styles.sectionTitle}>General</div>
        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>Application</div>
          <div className={styles.settingValue}>S9 Bug Tracker</div>
        </div>
        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>Version</div>
          <div className={styles.settingValue}>
            <span className={styles.versionTag}>0.1.0-dev</span>
          </div>
        </div>
      </div>

      <div className={styles.card}>
        <div className={styles.sectionTitle}>Authentication</div>
        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>Password Auth</div>
          <div className={styles.settingValue}>
            <span className={styles.enabledBadge}>Enabled</span>
          </div>
        </div>
        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>OIDC</div>
          <div className={styles.settingValue}>
            <span className={styles.infoText}>Configure via environment variables</span>
          </div>
        </div>
      </div>

      <div className={styles.card}>
        <div className={styles.sectionTitle}>Notifications</div>
        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>Email Notifications</div>
          <div className={styles.settingValue}>
            <span className={styles.infoText}>Configure SMTP via environment variables</span>
          </div>
        </div>
        <div className={styles.settingRow}>
          <div className={styles.settingLabel}>Batch Delay</div>
          <div className={styles.settingValue}>
            <span className={styles.monoValue}>120s</span>
          </div>
        </div>
      </div>

      <div className={styles.helpText}>
        System settings are configured via environment variables and CLI flags. See the deployment
        documentation for details.
      </div>
    </div>
  );
}
