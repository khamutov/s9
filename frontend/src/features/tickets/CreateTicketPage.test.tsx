import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router';
import { vi } from 'vitest';

vi.mock('../../api/tickets', () => ({
  createTicket: vi.fn(),
}));

vi.mock('../../api/components', () => ({
  listComponents: vi.fn(),
}));

vi.mock('../../api/milestones', () => ({
  listMilestones: vi.fn(),
}));

vi.mock('../../api/users', () => ({
  listUsers: vi.fn(),
}));

vi.mock('../../api/attachments', () => ({
  uploadAttachment: vi.fn(),
  attachmentUrl: vi.fn((id: number, name: string) => `/api/attachments/${id}/${name}`),
}));

import { createTicket } from '../../api/tickets';
import { listComponents } from '../../api/components';
import { listMilestones } from '../../api/milestones';
import { listUsers } from '../../api/users';
import CreateTicketPage from './CreateTicketPage';

const mockComponents = {
  items: [
    {
      id: 1,
      name: 'Platform',
      path: 'Platform',
      slug: 'PLAT',
      effective_slug: 'PLAT',
      owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      ticket_count: 5,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
    {
      id: 2,
      name: 'Auth',
      path: 'Platform / Auth',
      parent_id: 1,
      slug: null,
      effective_slug: 'PLAT',
      owner: { id: 2, login: 'maria', display_name: 'Maria Chen' },
      ticket_count: 3,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
};

const mockMilestones = {
  items: [
    {
      id: 1,
      name: 'v1.0 Launch',
      status: 'open',
      stats: {
        total: 10,
        new: 5,
        in_progress: 3,
        verify: 1,
        done: 1,
        estimated_hours: 40,
        remaining_hours: 20,
      },
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
};

const mockUsers = {
  items: [
    {
      id: 1,
      login: 'alex',
      display_name: 'Alex Kim',
      email: 'alex@s9.dev',
      role: 'admin',
      is_active: true,
      has_password: true,
      has_oidc: false,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
    {
      id: 2,
      login: 'maria',
      display_name: 'Maria Chen',
      email: 'maria@s9.dev',
      role: 'user',
      is_active: true,
      has_password: true,
      has_oidc: false,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
};

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/tickets/new']}>
        <Routes>
          <Route path="/tickets/new" element={<CreateTicketPage />} />
          <Route path="/tickets/:id" element={<div>Ticket detail</div>} />
          <Route path="/tickets" element={<div>Ticket list</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('CreateTicketPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listComponents).mockResolvedValue(mockComponents);
    vi.mocked(listMilestones).mockResolvedValue(mockMilestones);
    vi.mocked(listUsers).mockResolvedValue(mockUsers);
  });

  it('renders form with all fields', async () => {
    renderPage();

    expect(screen.getByLabelText(/title/i)).toBeInTheDocument();
    expect(screen.getByLabelText('Markdown editor')).toBeInTheDocument();
    expect(screen.getByLabelText(/type/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/priority/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/component/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/owner/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/milestone/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /create ticket/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument();
  });

  it('loads components into select', async () => {
    renderPage();

    await waitFor(() => {
      expect(screen.getByText('Platform')).toBeInTheDocument();
      expect(screen.getByText('Platform / Auth')).toBeInTheDocument();
    });
  });

  it('loads users into owner select', async () => {
    renderPage();

    await waitFor(() => {
      expect(screen.getByText('Alex Kim')).toBeInTheDocument();
      expect(screen.getByText('Maria Chen')).toBeInTheDocument();
    });
  });

  it('loads milestones into select', async () => {
    renderPage();

    await waitFor(() => {
      expect(screen.getByText('v1.0 Launch')).toBeInTheDocument();
    });
  });

  it('shows validation errors when submitting empty form', async () => {
    renderPage();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: /create ticket/i }));

    expect(screen.getByText('Title is required')).toBeInTheDocument();
    expect(screen.getByText('Component is required')).toBeInTheDocument();
    expect(screen.getByText('Owner is required')).toBeInTheDocument();
    expect(createTicket).not.toHaveBeenCalled();
  });

  it('clears title error when typing', async () => {
    renderPage();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: /create ticket/i }));
    expect(screen.getByText('Title is required')).toBeInTheDocument();

    await user.type(screen.getByLabelText(/title/i), 'Fix bug');
    expect(screen.queryByText('Title is required')).not.toBeInTheDocument();
  });

  it('submits valid form and navigates to ticket detail', async () => {
    const createdTicket = {
      id: 42,
      title: 'Fix login bug',
      slug: 'PLAT-42',
      type: 'bug' as const,
      status: 'new' as const,
      priority: 'P1' as const,
      owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      component: { id: 1, name: 'Platform', path: 'Platform', slug: 'PLAT' },
      created_by: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      cc: [],
      milestones: [],
      comment_count: 0,
      estimation_hours: null,
      estimation_display: null,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    };
    vi.mocked(createTicket).mockResolvedValue(createdTicket);
    renderPage();
    const user = userEvent.setup();

    // Wait for selects to load
    await waitFor(() => {
      expect(screen.getByText('Alex Kim')).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText(/title/i), 'Fix login bug');
    await user.selectOptions(screen.getByLabelText(/type/i), 'bug');
    await user.selectOptions(screen.getByLabelText(/priority/i), 'P1');
    await user.selectOptions(screen.getByLabelText(/component/i), '1');
    await user.selectOptions(screen.getByLabelText(/owner/i), '1');

    await user.click(screen.getByRole('button', { name: /create ticket/i }));

    await waitFor(() => {
      expect(createTicket).toHaveBeenCalledWith({
        title: 'Fix login bug',
        type: 'bug',
        priority: 'P1',
        component_id: 1,
        owner_id: 1,
      });
    });

    // Should navigate to the created ticket
    await waitFor(() => {
      expect(screen.getByText('Ticket detail')).toBeInTheDocument();
    });
  });

  it('includes description and milestone when provided', async () => {
    const createdTicket = {
      id: 43,
      title: 'New feature',
      slug: 'PLAT-43',
      type: 'feature' as const,
      status: 'new' as const,
      priority: 'P2' as const,
      owner: { id: 2, login: 'maria', display_name: 'Maria Chen' },
      component: { id: 1, name: 'Platform', path: 'Platform', slug: 'PLAT' },
      created_by: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      cc: [],
      milestones: [{ id: 1, name: 'v1.0 Launch' }],
      comment_count: 0,
      estimation_hours: null,
      estimation_display: null,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    };
    vi.mocked(createTicket).mockResolvedValue(createdTicket);
    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText('Maria Chen')).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText(/title/i), 'New feature');
    await user.type(screen.getByLabelText('Markdown editor'), 'Some description');
    await user.selectOptions(screen.getByLabelText(/component/i), '1');
    await user.selectOptions(screen.getByLabelText(/owner/i), '2');
    await user.selectOptions(screen.getByLabelText(/milestone/i), '1');

    await user.click(screen.getByRole('button', { name: /create ticket/i }));

    await waitFor(() => {
      expect(createTicket).toHaveBeenCalledWith({
        title: 'New feature',
        type: 'task',
        priority: 'P2',
        component_id: 1,
        owner_id: 2,
        description: 'Some description',
        milestones: [1],
      });
    });
  });

  it('navigates to ticket list on cancel', async () => {
    renderPage();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: /cancel/i }));

    expect(screen.getByText('Ticket list')).toBeInTheDocument();
  });

  it('shows server error on mutation failure', async () => {
    vi.mocked(createTicket).mockRejectedValue(new Error('Network error'));
    renderPage();
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText('Alex Kim')).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText(/title/i), 'Test');
    await user.selectOptions(screen.getByLabelText(/component/i), '1');
    await user.selectOptions(screen.getByLabelText(/owner/i), '1');
    await user.click(screen.getByRole('button', { name: /create ticket/i }));

    await waitFor(() => {
      expect(screen.getByText('Failed to create ticket. Please try again.')).toBeInTheDocument();
    });
  });

  it('defaults type to task and priority to P2', () => {
    renderPage();

    const typeSelect = screen.getByLabelText(/type/i) as HTMLSelectElement;
    const prioritySelect = screen.getByLabelText(/priority/i) as HTMLSelectElement;
    expect(typeSelect.value).toBe('task');
    expect(prioritySelect.value).toBe('P2');
  });
});
