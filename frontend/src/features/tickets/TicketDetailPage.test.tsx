import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router';
import { vi } from 'vitest';
import type { Ticket, Comment, ListResponse } from '../../api/types';
import { PageHeaderContext } from '../../components/layout/pageHeaderState';

vi.mock('../../api/tickets', () => ({
  getTicket: vi.fn(),
  updateTicket: vi.fn(),
}));

vi.mock('../../api/comments', () => ({
  listComments: vi.fn(),
}));

import { getTicket, updateTicket } from '../../api/tickets';
import { listComments } from '../../api/comments';
import TicketDetailPage from './TicketDetailPage';

const mockTicket = (overrides: Partial<Ticket> = {}): Ticket => ({
  id: 42,
  title: 'Crash on startup when config is missing',
  slug: 'S9-42',
  type: 'bug',
  status: 'in_progress',
  priority: 'P1',
  owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  component: { id: 5, name: 'DNS', path: '/Platform/Networking/DNS/' },
  created_by: { id: 2, login: 'maria', display_name: 'Maria Chen' },
  cc: [{ id: 3, login: 'bob', display_name: 'Bob Lee' }],
  milestones: [{ id: 1, name: 'v2.4' }],
  estimation_hours: 16,
  estimation_display: '2d',
  comment_count: 3,
  created_at: '2026-03-04T10:00:00.000Z',
  updated_at: '2026-03-06T14:30:00.000Z',
  ...overrides,
});

const mockComment = (overrides: Partial<Comment> = {}): Comment => ({
  id: 1,
  ticket_id: 42,
  number: 0,
  author: { id: 2, login: 'maria', display_name: 'Maria Chen' },
  body: 'This is the ticket description.',
  attachments: [],
  edit_count: 0,
  edits: [],
  created_at: '2026-03-04T10:00:00.000Z',
  updated_at: '2026-03-04T10:00:00.000Z',
  ...overrides,
});

function renderPage(ticketId = '42') {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const setConfig = vi.fn();

  return render(
    <QueryClientProvider client={queryClient}>
      <PageHeaderContext.Provider value={{ config: null, setConfig }}>
        <MemoryRouter initialEntries={[`/tickets/${ticketId}`]}>
          <Routes>
            <Route path="/tickets/:id" element={<TicketDetailPage />} />
            <Route path="/tickets" element={<div>Ticket list</div>} />
          </Routes>
        </MemoryRouter>
      </PageHeaderContext.Provider>
    </QueryClientProvider>,
  );
}

describe('TicketDetailPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state while fetching', () => {
    vi.mocked(getTicket).mockReturnValue(new Promise(() => {}));
    vi.mocked(listComments).mockReturnValue(new Promise(() => {}));
    renderPage();
    expect(screen.getByText('Loading ticket…')).toBeInTheDocument();
  });

  it('shows error state on fetch failure', async () => {
    vi.mocked(getTicket).mockRejectedValue(new Error('Not found'));
    vi.mocked(listComments).mockRejectedValue(new Error('Not found'));
    renderPage();
    expect(await screen.findByText('Failed to load ticket. Please try again.')).toBeInTheDocument();
  });

  it('renders ticket title and slug', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent(
      'Crash on startup when config is missing',
    );
    expect(screen.getAllByText('S9-42').length).toBeGreaterThanOrEqual(1);
  });

  it('renders status, priority, and type badges', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    // Status badge + metadata panel (inline select display)
    expect(screen.getAllByText('In Progress').length).toBeGreaterThanOrEqual(1);
    // Priority badge + metadata panel
    expect(screen.getAllByText('P1').length).toBeGreaterThanOrEqual(1);
    // Type badge + metadata panel
    expect(screen.getAllByText('Bug').length).toBeGreaterThanOrEqual(1);
  });

  it('renders metadata panel with owner, reporter, component', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    expect(screen.getByText('Alex Kim')).toBeInTheDocument();
    expect(screen.getByText('Maria Chen')).toBeInTheDocument();
    expect(screen.getByText('/Platform/Networking/DNS/')).toBeInTheDocument();
  });

  it('renders CC list in metadata', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    expect(screen.getByText('Bob Lee')).toBeInTheDocument();
  });

  it('renders milestone chip', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    expect(screen.getByText('v2.4')).toBeInTheDocument();
  });

  it('renders estimation value', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    expect(screen.getByText('2d')).toBeInTheDocument();
  });

  it('renders description from comment #0', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    const comments: ListResponse<Comment> = {
      items: [mockComment({ number: 0, body: 'This is the ticket description.' })],
    };
    vi.mocked(listComments).mockResolvedValue(comments);
    renderPage();

    expect(await screen.findByText('This is the ticket description.')).toBeInTheDocument();
  });

  it('renders activity comments (excluding #0)', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    const comments: ListResponse<Comment> = {
      items: [
        mockComment({ number: 0, body: 'Description text' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'I can reproduce this consistently.',
        }),
        mockComment({
          id: 3,
          number: 2,
          author: { id: 2, login: 'maria', display_name: 'Maria Chen' },
          body: 'That approach looks good.',
        }),
      ],
    };
    vi.mocked(listComments).mockResolvedValue(comments);
    renderPage();

    expect(await screen.findByText('I can reproduce this consistently.')).toBeInTheDocument();
    expect(screen.getByText('That approach looks good.')).toBeInTheDocument();
    // Activity count shows 2 (excluding description)
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('shows empty comments message when no activity', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    expect(await screen.findByText('No comments yet.')).toBeInTheDocument();
  });

  it('renders breadcrumb with link back to tickets', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    const ticketsLink = screen.getByRole('link', { name: 'Tickets' });
    expect(ticketsLink).toHaveAttribute('href', '/tickets');
  });

  it('renders created and updated dates', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    expect(screen.getByText('Mar 4, 2026')).toBeInTheDocument();
    expect(screen.getByText('Mar 6, 2026')).toBeInTheDocument();
  });

  // --- Inline editing tests ---

  it('opens status dropdown and changes status', async () => {
    const ticket = mockTicket();
    vi.mocked(getTicket).mockResolvedValue(ticket);
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    vi.mocked(updateTicket).mockResolvedValue({
      ...ticket,
      status: 'done',
    });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');

    // Click the status InlineSelect trigger in the metadata panel
    const statusBtn = screen.getByRole('button', { name: 'Status' });
    fireEvent.click(statusBtn);

    // Dropdown should appear with all status options
    expect(screen.getByRole('listbox')).toBeInTheDocument();

    // Select 'Done'
    fireEvent.click(screen.getByRole('option', { name: 'Done' }));

    await waitFor(() => {
      expect(updateTicket).toHaveBeenCalledWith(42, { status: 'done' });
    });
  });

  it('opens priority dropdown and changes priority', async () => {
    const ticket = mockTicket();
    vi.mocked(getTicket).mockResolvedValue(ticket);
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    vi.mocked(updateTicket).mockResolvedValue({
      ...ticket,
      priority: 'P0',
    });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');

    const priorityBtn = screen.getByRole('button', { name: 'Priority' });
    fireEvent.click(priorityBtn);
    // Pick P0 from dropdown — there are multiple P0 elements (option + display of other options)
    const p0Option = screen
      .getAllByRole('option')
      .find((el) => el.getAttribute('aria-selected') === 'false' && el.textContent?.includes('P0'));
    fireEvent.click(p0Option!);

    await waitFor(() => {
      expect(updateTicket).toHaveBeenCalledWith(42, { priority: 'P0' });
    });
  });

  it('edits estimation via inline text input', async () => {
    const ticket = mockTicket();
    vi.mocked(getTicket).mockResolvedValue(ticket);
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    vi.mocked(updateTicket).mockResolvedValue({
      ...ticket,
      estimation_display: '4d',
    });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');

    // Click the estimation display to enter edit mode
    const estimateBtn = screen.getByRole('button', { name: 'Edit Estimate' });
    fireEvent.click(estimateBtn);

    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: '4d' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => {
      expect(updateTicket).toHaveBeenCalledWith(42, { estimation: '4d' });
    });
  });
});
