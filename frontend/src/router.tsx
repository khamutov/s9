import { createBrowserRouter, Navigate } from 'react-router';
import RootLayout from './components/layout/RootLayout';

const router = createBrowserRouter([
  {
    path: '/',
    element: <RootLayout />,
    children: [
      { index: true, element: <Navigate to="/tickets" replace /> },
      { path: 'tickets', element: <div>Tickets</div> },
      { path: 'components', element: <div>Components</div> },
      { path: 'milestones', element: <div>Milestones</div> },
      { path: 'admin', element: <div>Admin</div> },
      { path: 'preferences', element: <div>Preferences</div> },
      { path: 'recent', element: <div>Recently Viewed</div> },
      { path: 'starred', element: <div>Starred</div> },
    ],
  },
  { path: '/login', element: <div>Login</div> },
]);

export default router;
