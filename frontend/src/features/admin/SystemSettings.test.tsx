import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, it, expect, vi } from 'vitest';
import SystemSettings from './SystemSettings';

vi.mock('../../components/layout/usePageHeader', () => ({
  usePageHeader: vi.fn(),
}));

const mockUseAuth = vi.fn();
vi.mock('../auth/useAuth', () => ({
  useAuth: () => mockUseAuth(),
}));

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <SystemSettings />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('SystemSettings', () => {
  it('renders settings sections for admin', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    renderPage();
    expect(screen.getByText('General')).toBeInTheDocument();
    expect(screen.getByText('Authentication')).toBeInTheDocument();
    expect(screen.getByText('Notifications')).toBeInTheDocument();
  });

  it('shows access denied for non-admin', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'user' } });
    renderPage();
    expect(screen.getByText(/administrator privileges/)).toBeInTheDocument();
    expect(screen.queryByText('General')).not.toBeInTheDocument();
  });

  it('displays version tag', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    renderPage();
    expect(screen.getByText('0.1.0-dev')).toBeInTheDocument();
  });

  it('shows auth configuration', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    renderPage();
    expect(screen.getByText('Enabled')).toBeInTheDocument();
    expect(screen.getByText('Password Auth')).toBeInTheDocument();
  });

  it('shows help text', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    renderPage();
    expect(
      screen.getByText(/configured via environment variables and CLI flags/),
    ).toBeInTheDocument();
  });
});
