import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router';
import { vi } from 'vitest';
import type { Ticket, Comment, ListResponse } from '../../api/types';

vi.mock('../../api/tickets', () => ({
  getTicket: vi.fn(),
  updateTicket: vi.fn(),
}));

vi.mock('../../api/comments', () => ({
  listComments: vi.fn(),
  createComment: vi.fn(),
  editComment: vi.fn(),
  deleteComment: vi.fn(),
}));

vi.mock('../../api/users', () => ({
  listCompactUsers: vi.fn(),
}));

vi.mock('../auth/useAuth', () => ({
  useAuth: vi.fn(() => ({
    user: { id: 1, login: 'alex', display_name: 'Alex Kim', role: 'user' },
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
  })),
}));

vi.mock('../../components/MarkdownEditor', () => ({
  MarkdownEditor: ({
    value,
    onChange,
    placeholder,
    disabled,
  }: {
    value: string;
    onChange: (v: string) => void;
    placeholder?: string;
    disabled?: boolean;
  }) => (
    <textarea
      data-testid="markdown-editor"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
    />
  ),
}));

import { getTicket, updateTicket } from '../../api/tickets';
import { listComments, createComment, editComment, deleteComment } from '../../api/comments';
import { listCompactUsers } from '../../api/users';
import { useAuth } from '../auth/useAuth';
import TicketDetailPage from './TicketDetailPage';

const MOCK_COMPACT_USERS = {
  items: [
    { id: 1, login: 'alex', display_name: 'Alex Kim' },
    { id: 2, login: 'maria', display_name: 'Maria Chen' },
    { id: 3, login: 'bob', display_name: 'Bob Lee' },
  ],
};

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
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[`/tickets/${ticketId}`]}>
        <Routes>
          <Route path="/tickets/:id" element={<TicketDetailPage />} />
          <Route path="/tickets" element={<div>Ticket list</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('TicketDetailPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useAuth).mockReturnValue({
      user: { id: 1, login: 'alex', display_name: 'Alex Kim', email: 'alex@s9.dev', role: 'user' },
      isLoading: false,
      login: vi.fn(),
      logout: vi.fn(),
    });
    vi.mocked(listCompactUsers).mockResolvedValue(MOCK_COMPACT_USERS);
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

    const input = screen.getByRole('textbox', { name: 'Estimate' });
    fireEvent.change(input, { target: { value: '4d' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => {
      expect(updateTicket).toHaveBeenCalledWith(42, { estimation: '4d' });
    });
  });

  // --- Comment thread tests ---

  it('renders comment form with editor and submit button', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');
    expect(screen.getByText('Add comment')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Comment' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Comment' })).toBeDisabled();
  });

  it('submits a new comment and clears form', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    vi.mocked(createComment).mockResolvedValue(
      mockComment({ id: 10, number: 1, body: 'New comment' }),
    );
    renderPage();

    await screen.findByText('Crash on startup when config is missing');

    // Type in the editor
    const editor = screen.getByTestId('markdown-editor');
    fireEvent.change(editor, { target: { value: 'New comment' } });

    // Submit button should be enabled
    const submitBtn = screen.getByRole('button', { name: 'Comment' });
    expect(submitBtn).not.toBeDisabled();
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(createComment).toHaveBeenCalledWith(42, { body: 'New comment' });
    });
  });

  it('shows edit button on own comments', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'My comment',
        }),
      ],
    });
    renderPage();

    await screen.findByText('My comment');
    expect(screen.getByRole('button', { name: 'Edit comment #1' })).toBeInTheDocument();
  });

  it('does not show edit button on other users comments', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 99, login: 'other', display_name: 'Other User' },
          body: 'Their comment',
        }),
      ],
    });
    renderPage();

    await screen.findByText('Their comment');
    expect(screen.queryByRole('button', { name: 'Edit comment #1' })).not.toBeInTheDocument();
  });

  it('opens edit form and saves edited comment', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'Original text',
        }),
      ],
    });
    vi.mocked(editComment).mockResolvedValue(
      mockComment({ id: 2, number: 1, body: 'Updated text', edit_count: 1 }),
    );
    renderPage();

    await screen.findByText('Original text');

    // Click edit
    fireEvent.click(screen.getByRole('button', { name: 'Edit comment #1' }));

    // Editor should appear with original text
    const editors = screen.getAllByTestId('markdown-editor');
    // Find the edit editor (not the comment form editor)
    const editEditor = editors.find((e) => (e as HTMLTextAreaElement).value === 'Original text')!;
    expect(editEditor).toBeInTheDocument();

    // Change the text
    fireEvent.change(editEditor, { target: { value: 'Updated text' } });

    // Save
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(editComment).toHaveBeenCalledWith(42, 1, { body: 'Updated text' });
    });
  });

  it('cancels edit and restores original text', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'Original text',
        }),
      ],
    });
    renderPage();

    await screen.findByText('Original text');
    fireEvent.click(screen.getByRole('button', { name: 'Edit comment #1' }));

    // Cancel editing
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    // Original text should be visible again (not in editor)
    expect(screen.getByText('Original text')).toBeInTheDocument();
  });

  it('shows edited tag on comments with edit_count > 0', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'Edited comment',
          edit_count: 2,
        }),
      ],
    });
    renderPage();

    await screen.findByText('Edited comment');
    expect(screen.getByText('edited')).toBeInTheDocument();
  });

  it('shows delete button for admin users', async () => {
    vi.mocked(useAuth).mockReturnValue({
      user: { id: 1, login: 'admin', display_name: 'Admin', email: 'admin@s9.dev', role: 'admin' },
      isLoading: false,
      login: vi.fn(),
      logout: vi.fn(),
    });
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 99, login: 'other', display_name: 'Other User' },
          body: 'Some comment',
        }),
      ],
    });
    renderPage();

    await screen.findByText('Some comment');
    expect(screen.getByRole('button', { name: 'Delete comment #1' })).toBeInTheDocument();
  });

  it('renders attachments on comment', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'See attached file.',
          attachments: [
            {
              id: 10,
              original_name: 'debug.log',
              mime_type: 'text/plain',
              size_bytes: 4096,
              url: '/api/attachments/10/debug.log',
            },
          ],
        }),
      ],
    });
    renderPage();

    await screen.findByText('See attached file.');
    expect(screen.getByText('Attachments')).toBeInTheDocument();
    expect(screen.getByText('debug.log')).toBeInTheDocument();
    expect(screen.getByText('4.0 KB')).toBeInTheDocument();
  });

  it('renders image attachments on description', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({
          number: 0,
          body: 'Description with screenshot.',
          attachments: [
            {
              id: 5,
              original_name: 'screenshot.png',
              mime_type: 'image/png',
              size_bytes: 245760,
              url: '/api/attachments/5/screenshot.png',
            },
          ],
        }),
      ],
    });
    renderPage();

    await screen.findByText('Description with screenshot.');
    expect(screen.getByAltText('screenshot.png')).toBeInTheDocument();
  });

  it('calls deleteComment when delete is clicked', async () => {
    vi.mocked(useAuth).mockReturnValue({
      user: { id: 1, login: 'admin', display_name: 'Admin', email: 'admin@s9.dev', role: 'admin' },
      isLoading: false,
      login: vi.fn(),
      logout: vi.fn(),
    });
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({ number: 0, body: 'Description' }),
        mockComment({
          id: 2,
          number: 1,
          author: { id: 99, login: 'other', display_name: 'Other User' },
          body: 'To delete',
        }),
      ],
    });
    vi.mocked(deleteComment).mockResolvedValue(undefined);
    renderPage();

    await screen.findByText('To delete');
    fireEvent.click(screen.getByRole('button', { name: 'Delete comment #1' }));

    await waitFor(() => {
      expect(deleteComment).toHaveBeenCalledWith(42, 1);
    });
  });

  // --- Inline title editing ---

  it('edits ticket title via inline text', async () => {
    const ticket = mockTicket();
    vi.mocked(getTicket).mockResolvedValue(ticket);
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    vi.mocked(updateTicket).mockResolvedValue({ ...ticket, title: 'Updated title' });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');

    // Click the title to enter edit mode
    const titleBtn = screen.getByRole('button', { name: 'Edit Title' });
    fireEvent.click(titleBtn);

    const input = screen.getByRole('textbox', { name: 'Title' });
    fireEvent.change(input, { target: { value: 'Updated title' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => {
      expect(updateTicket).toHaveBeenCalledWith(42, { title: 'Updated title' });
    });
  });

  // --- Inline owner editing ---

  it('changes owner via inline select dropdown', async () => {
    const ticket = mockTicket();
    vi.mocked(getTicket).mockResolvedValue(ticket);
    vi.mocked(listComments).mockResolvedValue({ items: [] });
    vi.mocked(updateTicket).mockResolvedValue({
      ...ticket,
      owner: { id: 2, login: 'maria', display_name: 'Maria Chen' },
    });
    renderPage();

    await screen.findByText('Crash on startup when config is missing');

    // Click the owner trigger
    const ownerBtn = screen.getByRole('button', { name: 'Owner' });
    fireEvent.click(ownerBtn);

    // Select Maria Chen
    const mariaOption = screen
      .getAllByRole('option')
      .find((el) => el.textContent?.includes('Maria Chen'));
    fireEvent.click(mariaOption!);

    await waitFor(() => {
      expect(updateTicket).toHaveBeenCalledWith(42, { owner_id: 2 });
    });
  });

  // --- Description editing ---

  it('shows edit button on description for author', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({
          number: 0,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'My description',
        }),
      ],
    });
    renderPage();

    await screen.findByText('My description');
    expect(screen.getByRole('button', { name: 'Edit description' })).toBeInTheDocument();
  });

  it('edits description (comment #0) and saves', async () => {
    vi.mocked(getTicket).mockResolvedValue(mockTicket());
    vi.mocked(listComments).mockResolvedValue({
      items: [
        mockComment({
          number: 0,
          author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
          body: 'Original description',
        }),
      ],
    });
    vi.mocked(editComment).mockResolvedValue(
      mockComment({ number: 0, body: 'Updated description', edit_count: 1 }),
    );
    renderPage();

    await screen.findByText('Original description');

    fireEvent.click(screen.getByRole('button', { name: 'Edit description' }));

    const editors = screen.getAllByTestId('markdown-editor');
    const descEditor = editors.find(
      (e) => (e as HTMLTextAreaElement).value === 'Original description',
    )!;
    fireEvent.change(descEditor, { target: { value: 'Updated description' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(editComment).toHaveBeenCalledWith(42, 0, { body: 'Updated description' });
    });
  });
});
