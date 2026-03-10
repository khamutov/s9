import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router';
import { vi } from 'vitest';
import type { Ticket, CursorPage } from '../../api/types';
import { PageHeaderContext } from '../../components/layout/pageHeaderState';

// Mock the API module
vi.mock('../../api/tickets', () => ({
  listTickets: vi.fn(),
}));

import { listTickets } from '../../api/tickets';
import TicketListPage from './TicketListPage';

const mockTicket = (overrides: Partial<Ticket> = {}): Ticket => ({
  id: 1,
  title: 'Test ticket',
  slug: 'S9-1',
  type: 'bug',
  status: 'new',
  priority: 'P2',
  owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  component: { id: 1, name: 'Auth', path: 'Auth', slug: 'AUTH' },
  created_by: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  cc: [],
  milestones: [],
  comment_count: 0,
  estimation_hours: null,
  estimation_display: null,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
  ...overrides,
});

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const setConfig = vi.fn();

  return render(
    <QueryClientProvider client={queryClient}>
      <PageHeaderContext.Provider value={{ config: null, setConfig }}>
        <MemoryRouter initialEntries={['/tickets']}>
          <Routes>
            <Route path="/tickets" element={<TicketListPage />} />
            <Route path="/tickets/:id" element={<div>Detail page</div>} />
            <Route path="/tickets/new" element={<div>Create page</div>} />
          </Routes>
        </MemoryRouter>
      </PageHeaderContext.Provider>
    </QueryClientProvider>,
  );
}

describe('TicketListPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state while fetching', () => {
    vi.mocked(listTickets).mockReturnValue(new Promise(() => {}));
    renderPage();
    expect(screen.getByText('Loading tickets…')).toBeInTheDocument();
  });

  it('shows error state on fetch failure', async () => {
    vi.mocked(listTickets).mockRejectedValue(new Error('Network error'));
    renderPage();
    expect(
      await screen.findByText('Failed to load tickets. Please try again.'),
    ).toBeInTheDocument();
  });

  it('shows empty state when no tickets', async () => {
    const response: CursorPage<Ticket> = {
      items: [],
      has_more: false,
    };
    vi.mocked(listTickets).mockResolvedValue(response);
    renderPage();
    expect(await screen.findByText('No tickets found.')).toBeInTheDocument();
  });

  it('renders ticket rows with correct data', async () => {
    const tickets = [
      mockTicket({ id: 1, title: 'Fix login bug', slug: 'AUTH-1', status: 'new', priority: 'P0' }),
      mockTicket({
        id: 2,
        title: 'Add dashboard',
        slug: 'S9-2',
        status: 'in_progress',
        priority: 'P2',
        owner: { id: 2, login: 'maria', display_name: 'Maria Chen' },
        component: { id: 2, name: 'Dashboard', path: 'Dashboard' },
      }),
    ];
    vi.mocked(listTickets).mockResolvedValue({
      items: tickets,
      has_more: false,
    });
    renderPage();

    expect(await screen.findByText('Fix login bug')).toBeInTheDocument();
    expect(screen.getByText('Add dashboard')).toBeInTheDocument();
    expect(screen.getByText('AUTH-1')).toBeInTheDocument();
    expect(screen.getByText('S9-2')).toBeInTheDocument();
    expect(screen.getByText('Alex Kim')).toBeInTheDocument();
    expect(screen.getByText('Maria Chen')).toBeInTheDocument();
    expect(screen.getByText('Auth')).toBeInTheDocument();
    expect(screen.getByText('Dashboard')).toBeInTheDocument();
  });

  it('displays status badges for each ticket', async () => {
    vi.mocked(listTickets).mockResolvedValue({
      items: [
        mockTicket({ id: 1, title: 'Ticket A', status: 'new' }),
        mockTicket({ id: 2, title: 'Ticket B', status: 'in_progress' }),
        mockTicket({ id: 3, title: 'Ticket C', status: 'done' }),
      ],
      has_more: false,
    });
    renderPage();

    await screen.findByText('Ticket A');
    // "New" appears in both badge and summary bar; "In Progress" appears twice too
    expect(screen.getAllByText('New').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('In Progress').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Done').length).toBeGreaterThanOrEqual(1);
  });

  it('shows summary bar with status counts', async () => {
    vi.mocked(listTickets).mockResolvedValue({
      items: [
        mockTicket({ id: 1, title: 'Ticket A', status: 'new' }),
        mockTicket({ id: 2, title: 'Ticket B', status: 'new' }),
        mockTicket({ id: 3, title: 'Ticket C', status: 'in_progress' }),
        mockTicket({ id: 4, title: 'Ticket D', status: 'done' }),
      ],
      has_more: false,
    });
    renderPage();

    await screen.findByText('Ticket A');

    // The summary bar renders status labels — check they exist
    const summaryElements = screen.getAllByText('New');
    // At least 1 from badge + 1 from summary
    expect(summaryElements.length).toBeGreaterThanOrEqual(2);
  });

  it('navigates to ticket detail on row click', async () => {
    vi.mocked(listTickets).mockResolvedValue({
      items: [mockTicket({ id: 42, title: 'Clickable ticket' })],
      has_more: false,
    });
    const user = userEvent.setup();
    renderPage();

    const row = await screen.findByText('Clickable ticket');
    await user.click(row.closest('tr')!);

    expect(screen.getByText('Detail page')).toBeInTheDocument();
  });

  it('renders Create Ticket link', async () => {
    vi.mocked(listTickets).mockResolvedValue({ items: [], has_more: false });
    renderPage();
    const link = screen.getByRole('link', { name: /create ticket/i });
    expect(link).toHaveAttribute('href', '/tickets/new');
  });

  it('selects rows with j/k keyboard shortcuts', async () => {
    vi.mocked(listTickets).mockResolvedValue({
      items: [
        mockTicket({ id: 1, title: 'First ticket' }),
        mockTicket({ id: 2, title: 'Second ticket' }),
        mockTicket({ id: 3, title: 'Third ticket' }),
      ],
      has_more: false,
    });
    const user = userEvent.setup();
    renderPage();

    await screen.findByText('First ticket');

    // Press j to select first row
    await user.keyboard('j');
    const rows = screen.getByText('First ticket').closest('tr')!;
    expect(rows.className).toContain('rowSelected');

    // Press j again to select second row
    await user.keyboard('j');
    const row2 = screen.getByText('Second ticket').closest('tr')!;
    expect(row2.className).toContain('rowSelected');
    expect(rows.className).not.toContain('rowSelected');

    // Press k to go back up
    await user.keyboard('k');
    expect(rows.className).toContain('rowSelected');
  });

  it('navigates to ticket detail with Enter on selected row', async () => {
    vi.mocked(listTickets).mockResolvedValue({
      items: [mockTicket({ id: 42, title: 'Enter ticket' })],
      has_more: false,
    });
    const user = userEvent.setup();
    renderPage();

    await screen.findByText('Enter ticket');

    // Select first row and press Enter
    await user.keyboard('j');
    await user.keyboard('{Enter}');

    expect(screen.getByText('Detail page')).toBeInTheDocument();
  });

  it('navigates to create page with c shortcut', async () => {
    vi.mocked(listTickets).mockResolvedValue({ items: [], has_more: false });
    const user = userEvent.setup();
    renderPage();

    await screen.findByText('No tickets found.');

    await user.keyboard('c');
    expect(screen.getByText('Create page')).toBeInTheDocument();
  });

  it('shows ticket count in header', async () => {
    vi.mocked(listTickets).mockResolvedValue({
      items: [
        mockTicket({ id: 1, title: 'Ticket A', status: 'new' }),
        mockTicket({ id: 2, title: 'Ticket B', status: 'new' }),
        mockTicket({ id: 3, title: 'Ticket C', status: 'new' }),
        mockTicket({ id: 4, title: 'Ticket D', status: 'new' }),
        mockTicket({ id: 5, title: 'Ticket E', status: 'new' }),
      ],
      has_more: false,
    });
    renderPage();
    await screen.findByText('Ticket A');
    // The heading contains the total count
    const heading = screen.getByRole('heading', { level: 1 });
    expect(heading).toHaveTextContent('Tickets');
    expect(heading).toHaveTextContent('5');
  });
});
