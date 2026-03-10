import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, it, expect, vi } from 'vitest';
import AdminPanel from './AdminPanel';

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
        <AdminPanel />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('AdminPanel', () => {
  it('renders navigation cards for admin user', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    renderPage();
    expect(screen.getByText('Users')).toBeInTheDocument();
    expect(screen.getByText('Components')).toBeInTheDocument();
    expect(screen.getByText('Settings')).toBeInTheDocument();
  });

  it('shows access denied for non-admin user', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'user' } });
    renderPage();
    expect(screen.getByText(/administrator privileges/)).toBeInTheDocument();
    expect(screen.queryByText('Users')).not.toBeInTheDocument();
  });

  it('renders card descriptions', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    renderPage();
    expect(screen.getByText(/Manage user accounts/)).toBeInTheDocument();
    expect(screen.getByText(/component tree/)).toBeInTheDocument();
    expect(screen.getByText(/System configuration/)).toBeInTheDocument();
  });

  it('cards link to correct routes', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    renderPage();
    const links = screen.getAllByRole('link');
    const hrefs = links.map((l) => l.getAttribute('href'));
    expect(hrefs).toContain('/admin/users');
    expect(hrefs).toContain('/admin/components');
    expect(hrefs).toContain('/admin/settings');
  });
});
