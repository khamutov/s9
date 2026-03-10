import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { AuthContext } from './AuthContext';
import AuthGuard from './AuthGuard';
import type { AuthState } from './authState';

const noop = () => Promise.resolve();

function renderWithAuth(authState: AuthState, initialRoute = '/protected') {
  return render(
    <AuthContext.Provider value={authState}>
      <MemoryRouter initialEntries={[initialRoute]}>
        <Routes>
          <Route element={<AuthGuard />}>
            <Route path="/protected" element={<div>Protected content</div>} />
          </Route>
          <Route path="/login" element={<div>Login page</div>} />
        </Routes>
      </MemoryRouter>
    </AuthContext.Provider>,
  );
}

describe('AuthGuard', () => {
  it('renders nothing while loading', () => {
    const { container } = renderWithAuth({
      user: null,
      isLoading: true,
      login: noop,
      logout: noop,
    });

    expect(container).toBeEmptyDOMElement();
  });

  it('redirects to /login when unauthenticated', () => {
    renderWithAuth({
      user: null,
      isLoading: false,
      login: noop,
      logout: noop,
    });

    expect(screen.getByText('Login page')).toBeInTheDocument();
  });

  it('renders child routes when authenticated', () => {
    renderWithAuth({
      user: {
        id: 1,
        login: 'admin',
        display_name: 'Admin',
        email: 'admin@example.com',
        role: 'admin',
      },
      isLoading: false,
      login: noop,
      logout: noop,
    });

    expect(screen.getByText('Protected content')).toBeInTheDocument();
  });
});
