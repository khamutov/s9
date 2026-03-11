import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import AccountPage from './AccountPage';

const mockRefreshUser = vi.fn().mockResolvedValue(undefined);
const mockUseAuth = vi.fn();
vi.mock('../auth/useAuth', () => ({
  useAuth: () => mockUseAuth(),
}));

vi.mock('../../api/users', () => ({
  updateUser: vi.fn(),
  setPassword: vi.fn(),
}));

import { updateUser, setPassword } from '../../api/users';

const mockUpdateUser = vi.mocked(updateUser);
const mockSetPassword = vi.mocked(setPassword);

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <AccountPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const TEST_USER = {
  id: 1,
  login: 'testuser',
  display_name: 'Test User',
  email: 'test@s9.dev',
  role: 'user' as const,
};

describe('AccountPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseAuth.mockReturnValue({ user: TEST_USER, refreshUser: mockRefreshUser });
  });

  it('renders profile section with user data', () => {
    renderPage();
    expect(screen.getByRole('heading', { name: 'Account' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Profile' })).toBeInTheDocument();
    expect(screen.getByText('testuser')).toBeInTheDocument();
    expect(screen.getByDisplayValue('Test User')).toBeInTheDocument();
    expect(screen.getByDisplayValue('test@s9.dev')).toBeInTheDocument();
  });

  it('renders password section', () => {
    renderPage();
    expect(screen.getByRole('heading', { name: 'Change Password' })).toBeInTheDocument();
    expect(screen.getByLabelText(/current password/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/^new password/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/confirm new password/i)).toBeInTheDocument();
  });

  it('shows login and role as read-only', () => {
    renderPage();
    expect(screen.getByText('testuser')).toBeInTheDocument();
    expect(screen.getByText('user')).toBeInTheDocument();
    // No inputs for login/role
    expect(screen.queryByLabelText(/^login$/i)).not.toBeInTheDocument();
  });

  it('validates empty display name on profile submit', async () => {
    const user = userEvent.setup();
    renderPage();
    const nameInput = screen.getByDisplayValue('Test User');
    await user.clear(nameInput);
    await user.click(screen.getByRole('button', { name: /save profile/i }));
    expect(screen.getByText('Display name is required')).toBeInTheDocument();
    expect(mockUpdateUser).not.toHaveBeenCalled();
  });

  it('validates empty email on profile submit', async () => {
    const user = userEvent.setup();
    renderPage();
    const emailInput = screen.getByDisplayValue('test@s9.dev');
    await user.clear(emailInput);
    await user.click(screen.getByRole('button', { name: /save profile/i }));
    expect(screen.getByText('Email is required')).toBeInTheDocument();
    expect(mockUpdateUser).not.toHaveBeenCalled();
  });

  it('submits profile update and refreshes user', async () => {
    mockUpdateUser.mockResolvedValueOnce({} as never);
    const user = userEvent.setup();
    renderPage();
    const nameInput = screen.getByDisplayValue('Test User');
    await user.clear(nameInput);
    await user.type(nameInput, 'New Name');
    await user.click(screen.getByRole('button', { name: /save profile/i }));
    await waitFor(() => {
      expect(mockUpdateUser).toHaveBeenCalledWith(1, {
        display_name: 'New Name',
        email: 'test@s9.dev',
      });
    });
    await waitFor(() => {
      expect(mockRefreshUser).toHaveBeenCalled();
    });
  });

  it('validates password fields', async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole('button', { name: /change password/i }));
    expect(screen.getByText('Current password is required')).toBeInTheDocument();
    expect(screen.getByText('New password is required')).toBeInTheDocument();
  });

  it('validates minimum password length', async () => {
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByLabelText(/current password/i), 'old123');
    await user.type(screen.getByLabelText(/^new password/i), 'short');
    await user.type(screen.getByLabelText(/confirm new password/i), 'short');
    await user.click(screen.getByRole('button', { name: /change password/i }));
    expect(screen.getByText('Password must be at least 8 characters')).toBeInTheDocument();
    expect(mockSetPassword).not.toHaveBeenCalled();
  });

  it('validates password confirmation match', async () => {
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByLabelText(/current password/i), 'oldpass123');
    await user.type(screen.getByLabelText(/^new password/i), 'newpass123');
    await user.type(screen.getByLabelText(/confirm new password/i), 'different123');
    await user.click(screen.getByRole('button', { name: /change password/i }));
    expect(screen.getByText('Passwords do not match')).toBeInTheDocument();
    expect(mockSetPassword).not.toHaveBeenCalled();
  });

  it('submits password change', async () => {
    mockSetPassword.mockResolvedValueOnce(undefined);
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByLabelText(/current password/i), 'oldpass123');
    await user.type(screen.getByLabelText(/^new password/i), 'newpass123');
    await user.type(screen.getByLabelText(/confirm new password/i), 'newpass123');
    await user.click(screen.getByRole('button', { name: /change password/i }));
    await waitFor(() => {
      expect(mockSetPassword).toHaveBeenCalledWith(1, {
        current_password: 'oldpass123',
        new_password: 'newpass123',
      });
    });
  });
});
