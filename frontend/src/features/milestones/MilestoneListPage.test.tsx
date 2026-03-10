import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import MilestoneListPage from './MilestoneListPage';
import type { Milestone, ListResponse } from '../../api/types';

vi.mock('../../components/layout/usePageHeader', () => ({
  usePageHeader: vi.fn(),
}));

vi.mock('../../api/milestones', () => ({
  listMilestones: vi.fn(),
}));

import { listMilestones } from '../../api/milestones';

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
      total: 4,
      new: 3,
      in_progress: 1,
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
    // v1.0 Launch has 5 Done, 1 Verify, 3 In Progress, 1 New
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
});
