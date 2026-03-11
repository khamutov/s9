import { Outlet } from 'react-router';
import Sidebar from './Sidebar';
import CommandBar from './CommandBar';
import styles from './RootLayout.module.css';

/** Top-level layout wrapping all authenticated pages. */
export default function RootLayout() {
  return (
    <div className={styles.app}>
      <Sidebar />
      <main className={styles.main}>
        <CommandBar />
        <Outlet />
      </main>
    </div>
  );
}
