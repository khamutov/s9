import { Outlet } from 'react-router';
import { PageHeaderProvider } from './PageHeaderContext';
import Sidebar from './Sidebar';
import CommandBar from './CommandBar';
import PageHeader from './PageHeader';
import styles from './RootLayout.module.css';

/** Top-level layout wrapping all authenticated pages. */
export default function RootLayout() {
  return (
    <PageHeaderProvider>
      <div className={styles.app}>
        <Sidebar />
        <main className={styles.main}>
          <CommandBar />
          <PageHeader />
          <Outlet />
        </main>
      </div>
    </PageHeaderProvider>
  );
}
