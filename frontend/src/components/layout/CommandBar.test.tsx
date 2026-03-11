import { render, screen, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import CommandBar from './CommandBar';
import type { Ticket, SearchResult } from '../../api/types';

const mockNavigate = vi.fn();
vi.mock('react-router', async () => {
  const actual = await vi.importActual('react-router');
  return { ...actual, useNavigate: () => mockNavigate };
});

vi.mock('../../api/tickets', () => ({
  listTickets: vi.fn(),
}));

import { listTickets } from '../../api/tickets';

const SEARCH_RESULTS: SearchResult<Ticket> = {
  items: [
    {
      id: 42,
      slug: 'PLAT-42',
      type: 'bug',
      title: 'Crash on startup when config is missing',
      status: 'new',
      priority: 'P1',
      owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      component: { id: 5, name: 'DNS', path: '/Platform/DNS/', effective_slug: 'PLAT' },
      created_by: { id: 2, login: 'maria', display_name: 'Maria Chen' },
      cc: [],
      milestones: [],
      comment_count: 1,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-03-01T00:00:00Z',
    },
    {
      id: 15,
      slug: null,
      type: 'feature',
      title: 'Add dark mode config toggle',
      status: 'in_progress',
      priority: 'P3',
      owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      component: { id: 2, name: 'UI', path: '/UI/', effective_slug: null },
      created_by: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      cc: [],
      milestones: [],
      comment_count: 0,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-02-01T00:00:00Z',
    },
  ],
  total: 2,
  page: 1,
  page_size: 8,
};

function renderCommandBar() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <CommandBar />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('CommandBar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(listTickets).mockResolvedValue(SEARCH_RESULTS);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders search input with placeholder', () => {
    renderCommandBar();
    expect(screen.getByPlaceholderText('Search or jump to...')).toBeInTheDocument();
  });

  it('shows keyboard shortcut hint', () => {
    renderCommandBar();
    expect(screen.getByText('⌘K')).toBeInTheDocument();
  });

  it('searches tickets after debounce', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    const input = screen.getByPlaceholderText('Search or jump to...');
    await user.type(input, 'crash');
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(listTickets).toHaveBeenCalledWith({ q: 'crash', page_size: 8 });
  });

  it('displays search results in dropdown', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    await user.type(screen.getByPlaceholderText('Search or jump to...'), 'crash');
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(await screen.findByText('PLAT-42')).toBeInTheDocument();
    expect(screen.getByText('Crash on startup when config is missing')).toBeInTheDocument();
    expect(screen.getByText('#15')).toBeInTheDocument();
  });

  it('navigates to ticket on click', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    await user.type(screen.getByPlaceholderText('Search or jump to...'), 'crash');
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    await screen.findByText('PLAT-42');
    await user.click(screen.getByText('Crash on startup when config is missing'));

    expect(mockNavigate).toHaveBeenCalledWith('/tickets/42');
  });

  it('navigates to ticket with keyboard (ArrowDown + Enter)', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    const input = screen.getByPlaceholderText('Search or jump to...');
    await user.type(input, 'crash');
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    await screen.findByText('PLAT-42');
    await user.keyboard('{ArrowDown}{Enter}');

    expect(mockNavigate).toHaveBeenCalledWith('/tickets/42');
  });

  it('shows "No tickets found" for empty results', async () => {
    vi.mocked(listTickets).mockResolvedValue({ items: [], total: 0, page: 1, page_size: 8 });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    await user.type(screen.getByPlaceholderText('Search or jump to...'), 'zzz');
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(await screen.findByText('No tickets found')).toBeInTheDocument();
  });

  it('closes dropdown on Escape', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    await user.type(screen.getByPlaceholderText('Search or jump to...'), 'crash');
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    await screen.findByText('PLAT-42');
    await user.keyboard('{Escape}');

    expect(screen.queryByText('PLAT-42')).not.toBeInTheDocument();
  });

  it('does not search when input is empty', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    const input = screen.getByPlaceholderText('Search or jump to...');
    await user.type(input, 'a');
    await user.clear(input);
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(listTickets).not.toHaveBeenCalled();
  });

  it('clears input after navigating to ticket', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderCommandBar();

    const input = screen.getByPlaceholderText('Search or jump to...');
    await user.type(input, 'crash');
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    await screen.findByText('PLAT-42');
    await user.click(screen.getByText('Crash on startup when config is missing'));

    expect(input).toHaveValue('');
  });
});
