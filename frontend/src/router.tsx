import { lazy } from 'react';
import { createBrowserRouter, Navigate } from 'react-router';
import RootLayout from './components/layout/RootLayout';
import AuthGuard from './features/auth/AuthGuard';
import Lazy from './components/Lazy';

// Route-level code splitting via React.lazy() per DD §7/§19
const LoginPage = lazy(() => import('./features/auth/LoginPage'));
const TicketListPage = lazy(() => import('./features/tickets/TicketListPage'));
const CreateTicketPage = lazy(() => import('./features/tickets/CreateTicketPage'));
const TicketDetailPage = lazy(() => import('./features/tickets/TicketDetailPage'));
const ComponentTreePage = lazy(() => import('./features/components/ComponentTreePage'));
const MilestoneListPage = lazy(() => import('./features/milestones/MilestoneListPage'));
const MilestoneDetailPage = lazy(() => import('./features/milestones/MilestoneDetailPage'));
const AccountPage = lazy(() => import('./features/account/AccountPage'));
const AdminPanel = lazy(() => import('./features/admin/AdminPanel'));
const UserManagement = lazy(() => import('./features/admin/UserManagement'));
const ComponentManagement = lazy(() => import('./features/admin/ComponentManagement'));
const SystemSettings = lazy(() => import('./features/admin/SystemSettings'));

const router = createBrowserRouter([
  {
    path: '/login',
    element: (
      <Lazy>
        <LoginPage />
      </Lazy>
    ),
  },
  {
    path: '/',
    element: <AuthGuard />,
    children: [
      {
        element: <RootLayout />,
        children: [
          { index: true, element: <Navigate to="/tickets" replace /> },
          {
            path: 'tickets',
            element: (
              <Lazy>
                <TicketListPage />
              </Lazy>
            ),
          },
          {
            path: 'tickets/new',
            element: (
              <Lazy>
                <CreateTicketPage />
              </Lazy>
            ),
          },
          {
            path: 'tickets/:id',
            element: (
              <Lazy>
                <TicketDetailPage />
              </Lazy>
            ),
          },
          {
            path: 'components',
            element: (
              <Lazy>
                <ComponentTreePage />
              </Lazy>
            ),
          },
          {
            path: 'milestones',
            element: (
              <Lazy>
                <MilestoneListPage />
              </Lazy>
            ),
          },
          {
            path: 'milestones/:id',
            element: (
              <Lazy>
                <MilestoneDetailPage />
              </Lazy>
            ),
          },
          {
            path: 'account',
            element: (
              <Lazy>
                <AccountPage />
              </Lazy>
            ),
          },
          {
            path: 'admin',
            element: (
              <Lazy>
                <AdminPanel />
              </Lazy>
            ),
          },
          {
            path: 'admin/users',
            element: (
              <Lazy>
                <UserManagement />
              </Lazy>
            ),
          },
          {
            path: 'admin/components',
            element: (
              <Lazy>
                <ComponentManagement />
              </Lazy>
            ),
          },
          {
            path: 'admin/settings',
            element: (
              <Lazy>
                <SystemSettings />
              </Lazy>
            ),
          },
        ],
      },
    ],
  },
]);

export default router;
