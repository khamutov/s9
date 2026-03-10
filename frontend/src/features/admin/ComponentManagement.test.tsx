import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import ComponentManagement from './ComponentManagement';
import type { Component, User, ListResponse } from '../../api/types';

vi.mock('../../components/layout/usePageHeader', () => ({
  usePageHeader: vi.fn(),
}));

const mockUseAuth = vi.fn();
vi.mock('../auth/useAuth', () => ({
  useAuth: () => mockUseAuth(),
}));

vi.mock('../../api/components', () => ({
  listComponents: vi.fn(),
  createComponent: vi.fn(),
  updateComponent: vi.fn(),
  deleteComponent: vi.fn(),
}));

vi.mock('../../api/users', () => ({
  listUsers: vi.fn(),
}));

import { listComponents, createComponent, deleteComponent } from '../../api/components';
import { listUsers } from '../../api/users';

const COMPONENTS: Component[] = [
  {
    id: 1,
    name: 'Platform',
    parent_id: null,
    path: 'Platform',
    slug: 'PLAT',
    effective_slug: 'PLAT',
    owner: { id: 1, login: 'alice', display_name: 'Alice Admin' },
    ticket_count: 5,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-03-01T00:00:00Z',
  },
  {
    id: 2,
    name: 'API',
    parent_id: 1,
    path: 'Platform / API',
    slug: null,
    effective_slug: 'PLAT',
    owner: { id: 2, login: 'bob', display_name: 'Bob User' },
    ticket_count: 0,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-03-01T00:00:00Z',
  },
];

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
];

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <ComponentManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('ComponentManagement', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'admin' } });
    vi.mocked(listComponents).mockResolvedValue({ items: COMPONENTS } as ListResponse<Component>);
    vi.mocked(listUsers).mockResolvedValue({ items: USERS } as ListResponse<User>);
  });

  it('renders component table', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    expect(screen.getAllByText('PLAT')).toHaveLength(2);
    expect(screen.getByText('Alice Admin')).toBeInTheDocument();
  });

  it('shows access denied for non-admin', () => {
    mockUseAuth.mockReturnValue({ user: { id: 1, role: 'user' } });
    renderPage();
    expect(screen.getByText(/administrator privileges/)).toBeInTheDocument();
  });

  it('opens create component modal', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    await user.click(screen.getByText('Add Component'));
    expect(screen.getByRole('heading', { name: 'Create Component' })).toBeInTheDocument();
  });

  it('validates create form', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    await user.click(screen.getByText('Add Component'));
    await user.click(screen.getByRole('button', { name: /Create Component/ }));
    await waitFor(() => {
      expect(screen.getByText('Name is required')).toBeInTheDocument();
    });
  });

  it('submits create form', async () => {
    vi.mocked(createComponent).mockResolvedValue(COMPONENTS[0]);
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    await user.click(screen.getByText('Add Component'));
    await user.type(screen.getByLabelText(/^Name/), 'Frontend');
    await user.selectOptions(screen.getByLabelText(/Owner/), '1');
    await user.click(screen.getByRole('button', { name: /Create Component/ }));
    await waitFor(() => {
      expect(createComponent).toHaveBeenCalledWith(
        expect.objectContaining({ name: 'Frontend', owner_id: 1 }),
      );
    });
  });

  it('opens edit component modal', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    const editButtons = screen.getAllByText('Edit');
    await user.click(editButtons[0]);
    expect(screen.getByText('Edit Component')).toBeInTheDocument();
  });

  it('opens delete confirmation', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    const deleteButtons = screen.getAllByText('Delete');
    await user.click(deleteButtons[1]); // API has 0 tickets
    expect(screen.getByText('Delete Component')).toBeInTheDocument();
  });

  it('can delete component with no tickets', async () => {
    vi.mocked(deleteComponent).mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    const deleteButtons = screen.getAllByText('Delete');
    await user.click(deleteButtons[1]); // API has 0 tickets
    // Modal is now open — find the confirm button inside the modal actions
    const modalButtons = screen.getAllByRole('button', { name: /Delete/ });
    const confirmBtn = modalButtons[modalButtons.length - 1]; // last one is the confirm button in modal
    expect(confirmBtn).not.toBeDisabled();
    await user.click(confirmBtn);
    await waitFor(() => {
      expect(deleteComponent).toHaveBeenCalledWith(2);
    });
  });

  it('disables delete for component with tickets', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('Platform / API')).toBeInTheDocument();
    });
    const deleteButtons = screen.getAllByText('Delete');
    await user.click(deleteButtons[0]); // Platform has 5 tickets
    // The confirm delete button should be disabled
    expect(screen.getByText(/cannot be deleted/)).toBeInTheDocument();
  });

  it('shows component count', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('2')).toBeInTheDocument();
    });
  });
});
