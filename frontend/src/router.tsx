import { createBrowserRouter, Navigate } from 'react-router';
import App from './App';

// Placeholder routes — replaced with real page components in Phase 5.
const router = createBrowserRouter([
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <Navigate to="/tickets" replace /> },
      { path: 'tickets', element: <div>Tickets</div> },
    ],
  },
  { path: '/login', element: <div>Login</div> },
]);

export default router;
