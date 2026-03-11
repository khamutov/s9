import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import MilestoneListPage from './MilestoneListPage';
import type { Milestone, ListResponse } from '../../api/types';

vi.mock('../../api/milestones', () => ({
  listMilestones: vi.fn(),
  createMilestone: vi.fn(),
  updateMilestone: vi.fn(),
  deleteMilestone: vi.fn(),
}));

vi.mock('../auth/useAuth', () => ({
  useAuth: vi.fn(),
}));

import {
  listMilestones,
  createMilestone,
  updateMilestone,
  deleteMilestone,
} from '../../api/milestones';
import { useAuth } from '../auth/useAuth';

const MILESTONES: Milestone[] = [
  {
    id: 1,
    name: 'v1.0 Launch',
    description: 'Core platform release.',
    due_date: '2026-04-01',
    status: 'open',
    stats: {
      total: 10,
      new: 1,
      in_progress: 3,
      verify: 1,
      done: 5,
      estimated_hours: 40,
      remaining_hours: 20,
    },
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-03-01T00:00:00Z',
  },
  {
    id: 2,
    name: 'Backlog',
    description: null,
    due_date: null,
    status: 'open',
    stats: {
      total: 0,
      new: 0,
      in_progress: 0,
      verify: 0,
      done: 0,
      estimated_hours: 0,
      remaining_hours: 0,
    },
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
  },
];

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <MilestoneListPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('MilestoneListPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    const response: ListResponse<Milestone> = { items: MILESTONES };
    vi.mocked(listMilestones).mockResolvedValue(response);
    vi.mocked(useAuth).mockReturnValue({
      user: { id: 1, login: 'admin', display_name: 'Admin', email: 'a@b.c', role: 'admin' },
      isLoading: false,
      login: vi.fn(),
      logout: vi.fn(),
      refresh: vi.fn(),
    });
  });

  it('renders milestone cards with names', async () => {
    renderPage();
    expect(await screen.findByText('v1.0 Launch')).toBeInTheDocument();
    expect(screen.getByText('Backlog')).toBeInTheDocument();
  });

  it('shows milestone count in page header', async () => {
    renderPage();
    expect(await screen.findByText('2')).toBeInTheDocument();
  });

  it('displays description when present', async () => {
    renderPage();
    expect(await screen.findByText('Core platform release.')).toBeInTheDocument();
  });

  it('shows done percentage in progress bar', async () => {
    renderPage();
    expect(await screen.findByText('50%')).toBeInTheDocument();
    expect(screen.getByText('0%')).toBeInTheDocument();
  });

  it('shows stats row with ticket counts', async () => {
    renderPage();
    const cards = await screen.findAllByText('Done');
    expect(cards.length).toBeGreaterThanOrEqual(2);
  });

  it('shows due date for milestones with one', async () => {
    renderPage();
    expect(await screen.findByText(/Due Apr/)).toBeInTheDocument();
  });

  it('shows dash for milestones without due date', async () => {
    renderPage();
    expect(await screen.findByText('\u2014')).toBeInTheDocument();
  });

  it('shows status badges', async () => {
    renderPage();
    const badges = await screen.findAllByText('Open');
    expect(badges.length).toBe(2);
  });

  it('filters milestones by name', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('v1.0 Launch');
    const filterInput = screen.getByPlaceholderText('Filter milestones...');
    await user.type(filterInput, 'Backlog');
    expect(screen.queryByText('v1.0 Launch')).not.toBeInTheDocument();
    expect(screen.getByText('Backlog')).toBeInTheDocument();
  });

  it('filters milestones by description', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('v1.0 Launch');
    const filterInput = screen.getByPlaceholderText('Filter milestones...');
    await user.type(filterInput, 'platform');
    expect(screen.getByText('v1.0 Launch')).toBeInTheDocument();
    expect(screen.queryByText('Backlog')).not.toBeInTheDocument();
  });

  it('shows empty state when no milestones match filter', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('v1.0 Launch');
    const filterInput = screen.getByPlaceholderText('Filter milestones...');
    await user.type(filterInput, 'zzz-no-match');
    expect(screen.getByText('No milestones found.')).toBeInTheDocument();
  });

  it('shows loading state', () => {
    vi.mocked(listMilestones).mockReturnValue(new Promise(() => {}));
    renderPage();
    expect(screen.getByText(/Loading milestones/)).toBeInTheDocument();
  });

  it('shows error state', async () => {
    vi.mocked(listMilestones).mockRejectedValue(new Error('fail'));
    renderPage();
    expect(await screen.findByText(/Failed to load milestones/)).toBeInTheDocument();
  });

  it('has View Tickets link for each milestone', async () => {
    renderPage();
    const links = await screen.findAllByRole('link', { name: 'View Tickets' });
    expect(links).toHaveLength(2);
    expect(links[0]).toHaveAttribute('href', '/tickets?q=milestone:v1.0%20Launch');
  });

  describe('admin CRUD', () => {
    it('shows Create Milestone button for admin', async () => {
      renderPage();
      expect(await screen.findByRole('button', { name: /Create Milestone/ })).toBeInTheDocument();
    });

    it('hides Create Milestone button for non-admin', async () => {
      vi.mocked(useAuth).mockReturnValue({
        user: { id: 2, login: 'user', display_name: 'User', email: 'u@b.c', role: 'user' },
        isLoading: false,
        login: vi.fn(),
        logout: vi.fn(),
        refresh: vi.fn(),
      });
      renderPage();
      await screen.findByText('v1.0 Launch');
      expect(screen.queryByRole('button', { name: /Create Milestone/ })).not.toBeInTheDocument();
    });

    it('shows Edit and Delete buttons for admin', async () => {
      renderPage();
      const editBtns = await screen.findAllByRole('button', { name: 'Edit' });
      const deleteBtns = screen.getAllByRole('button', { name: 'Delete' });
      expect(editBtns).toHaveLength(2);
      expect(deleteBtns).toHaveLength(2);
    });

    it('hides Edit and Delete buttons for non-admin', async () => {
      vi.mocked(useAuth).mockReturnValue({
        user: { id: 2, login: 'user', display_name: 'User', email: 'u@b.c', role: 'user' },
        isLoading: false,
        login: vi.fn(),
        logout: vi.fn(),
        refresh: vi.fn(),
      });
      renderPage();
      await screen.findByText('v1.0 Launch');
      expect(screen.queryByRole('button', { name: 'Edit' })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
    });

    it('opens create modal and submits', async () => {
      const user = userEvent.setup();
      vi.mocked(createMilestone).mockResolvedValue({ ...MILESTONES[0], id: 3, name: 'v2.0' });
      renderPage();
      await screen.findByText('v1.0 Launch');

      await user.click(screen.getByRole('button', { name: /Create Milestone/ }));
      const dialog = screen.getByRole('dialog', { name: 'Create Milestone' });
      expect(dialog).toBeInTheDocument();

      await user.type(within(dialog).getByLabelText(/Name/), 'v2.0');
      await user.type(within(dialog).getByLabelText(/Description/), 'Next release');
      await user.click(within(dialog).getByRole('button', { name: /Create Milestone/ }));

      expect(createMilestone).toHaveBeenCalledWith({
        name: 'v2.0',
        description: 'Next release',
      });
    });

    it('validates name is required on create', async () => {
      const user = userEvent.setup();
      renderPage();
      await screen.findByText('v1.0 Launch');

      await user.click(screen.getByRole('button', { name: /Create Milestone/ }));
      const dialog = screen.getByRole('dialog', { name: 'Create Milestone' });
      await user.click(within(dialog).getByRole('button', { name: /Create Milestone/ }));

      expect(within(dialog).getByText('Name is required')).toBeInTheDocument();
      expect(createMilestone).not.toHaveBeenCalled();
    });

    it('opens edit modal with pre-filled data', async () => {
      const user = userEvent.setup();
      renderPage();
      const editBtns = await screen.findAllByRole('button', { name: 'Edit' });

      await user.click(editBtns[0]);
      const dialog = screen.getByRole('dialog', { name: 'Edit Milestone' });
      expect(dialog).toBeInTheDocument();

      expect(within(dialog).getByLabelText(/Name/)).toHaveValue('v1.0 Launch');
      expect(within(dialog).getByLabelText(/Description/)).toHaveValue('Core platform release.');
      expect(within(dialog).getByLabelText(/Due date/)).toHaveValue('2026-04-01');
    });

    it('submits edit form', async () => {
      const user = userEvent.setup();
      vi.mocked(updateMilestone).mockResolvedValue({ ...MILESTONES[0], name: 'v1.0 GA' });
      renderPage();
      const editBtns = await screen.findAllByRole('button', { name: 'Edit' });

      await user.click(editBtns[0]);
      const dialog = screen.getByRole('dialog', { name: 'Edit Milestone' });
      const nameInput = within(dialog).getByLabelText(/Name/);
      await user.clear(nameInput);
      await user.type(nameInput, 'v1.0 GA');
      await user.click(within(dialog).getByRole('button', { name: /Save Changes/ }));

      expect(updateMilestone).toHaveBeenCalledWith(1, {
        name: 'v1.0 GA',
        description: 'Core platform release.',
        due_date: '2026-04-01',
        status: 'open',
      });
    });

    it('opens delete modal with milestone name', async () => {
      const user = userEvent.setup();
      renderPage();
      const deleteBtns = await screen.findAllByRole('button', { name: 'Delete' });

      await user.click(deleteBtns[1]); // Backlog (0 tickets)
      const dialog = screen.getByRole('dialog', { name: 'Delete Milestone' });
      expect(within(dialog).getByText('Backlog')).toBeInTheDocument();
    });

    it('disables delete button when milestone has tickets', async () => {
      const user = userEvent.setup();
      renderPage();
      const deleteBtns = await screen.findAllByRole('button', { name: 'Delete' });

      await user.click(deleteBtns[0]); // v1.0 Launch (10 tickets)
      const dialog = screen.getByRole('dialog', { name: 'Delete Milestone' });
      expect(within(dialog).getByText(/10 assigned tickets/)).toBeInTheDocument();
      expect(within(dialog).getByRole('button', { name: 'Delete' })).toBeDisabled();
    });

    it('submits delete when no tickets assigned', async () => {
      const user = userEvent.setup();
      vi.mocked(deleteMilestone).mockResolvedValue(undefined);
      renderPage();
      const deleteBtns = await screen.findAllByRole('button', { name: 'Delete' });

      await user.click(deleteBtns[1]); // Backlog (0 tickets)
      const dialog = screen.getByRole('dialog', { name: 'Delete Milestone' });
      await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

      expect(deleteMilestone).toHaveBeenCalledWith(2);
    });

    it('closes modal on cancel', async () => {
      const user = userEvent.setup();
      renderPage();
      await screen.findByText('v1.0 Launch');

      await user.click(screen.getByRole('button', { name: /Create Milestone/ }));
      expect(screen.getByRole('dialog', { name: 'Create Milestone' })).toBeInTheDocument();

      await user.click(screen.getByRole('button', { name: 'Cancel' }));
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
  });
});
