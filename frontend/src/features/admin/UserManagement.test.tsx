import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import UserManagement from './UserManagement';
import type { User, ListResponse } from '../../api/types';

const mockUseAuth = vi.fn();
vi.mock('../auth/useAuth', () => ({
  useAuth: () => mockUseAuth(),
}));

vi.mock('../../api/users', () => ({
  listUsers: vi.fn(),
  createUser: vi.fn(),
  updateUser: vi.fn(),
  setPassword: vi.fn(),
}));

import { listUsers, createUser, updateUser } from '../../api/users';

const USERS: User[] = [
  {
    id: 1,
    login: 'alice',
    display_name: 'Alice Admin',
    email: 'alice@s9.dev',
    role: 'admin',
    is_active: true,
    has_password: true,
    has_oidc: false,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-03-01T00:00:00Z',
  },
  {
    id: 2,
    login: 'bob',
    display_name: 'Bob User',
    email: 'bob@s9.dev',
    role: 'user',
    is_active: true,
    has_password: true,
    has_oidc: true,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-03-01T00:00:00Z',
  },
];

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <UserManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('UserManagement', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    vi.mocked(listUsers).mockResolvedValue({ items: USERS } as ListResponse<User>);
  });

  it('renders user table with data', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('alice')).toBeInTheDocument();
    });
    expect(screen.getByText('Bob User')).toBeInTheDocument();
    expect(screen.getByText('bob@s9.dev')).toBeInTheDocument();
  });

  it('shows access denied for non-admin', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'user' } });
    renderPage();
    expect(screen.getByText(/administrator privileges/)).toBeInTheDocument();
  });

  it('shows role badges', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('admin')).toBeInTheDocument();
    });
    expect(screen.getByText('user')).toBeInTheDocument();
  });

  it('shows auth tags', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getAllByText('password')).toHaveLength(2);
    });
    expect(screen.getByText('oidc')).toBeInTheDocument();
  });

  it('opens create user modal', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('alice')).toBeInTheDocument();
    });
    await user.click(screen.getByText('Add User'));
    expect(screen.getByRole('heading', { name: 'Create User' })).toBeInTheDocument();
    expect(screen.getByLabelText(/Login/)).toBeInTheDocument();
  });

  it('validates create form', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('alice')).toBeInTheDocument();
    });
    await user.click(screen.getByText('Add User'));
    await user.click(screen.getByRole('button', { name: /Create User/ }));
    // Wait for validation errors
    await waitFor(() => {
      expect(screen.getByText('Login is required')).toBeInTheDocument();
    });
  });

  it('submits create form', async () => {
    vi.mocked(createUser).mockResolvedValue(USERS[0]);
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('alice')).toBeInTheDocument();
    });
    await user.click(screen.getByText('Add User'));
    await user.type(screen.getByLabelText(/Login/), 'newuser');
    await user.type(screen.getByLabelText(/Display Name/), 'New User');
    await user.type(screen.getByLabelText(/Email/), 'new@s9.dev');
    await user.click(screen.getByRole('button', { name: /Create User/ }));
    await waitFor(() => {
      expect(createUser).toHaveBeenCalledWith(
        expect.objectContaining({
          login: 'newuser',
          display_name: 'New User',
          email: 'new@s9.dev',
          role: 'user',
        }),
      );
    });
  });

  it('opens edit user modal', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('alice')).toBeInTheDocument();
    });
    const editButtons = screen.getAllByText('Edit');
    await user.click(editButtons[0]);
    expect(screen.getByText('Edit User')).toBeInTheDocument();
  });

  it('submits edit form', async () => {
    vi.mocked(updateUser).mockResolvedValue(USERS[0]);
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('alice')).toBeInTheDocument();
    });
    const editButtons = screen.getAllByText('Edit');
    await user.click(editButtons[0]);
    const nameInput = screen.getByLabelText(/Display Name/);
    await user.clear(nameInput);
    await user.type(nameInput, 'Alice Updated');
    await user.click(screen.getByText('Save Changes'));
    await waitFor(() => {
      expect(updateUser).toHaveBeenCalledWith(
        1,
        expect.objectContaining({ display_name: 'Alice Updated' }),
      );
    });
  });

  it('opens password modal', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('alice')).toBeInTheDocument();
    });
    const pwButtons = screen.getAllByText('Password');
    await user.click(pwButtons[0]);
    expect(screen.getByRole('heading', { name: 'Set Password' })).toBeInTheDocument();
  });

  it('shows user count', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });
  });
});
