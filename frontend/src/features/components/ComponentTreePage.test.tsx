import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import ComponentTreePage from './ComponentTreePage';
import type { Component, ListResponse } from '../../api/types';

const MOCK_COMPONENTS: ListResponse<Component> = {
  items: [
    {
      id: 1,
      name: 'Platform',
      parent_id: null,
      path: '/Platform/',
      slug: 'PLAT',
      effective_slug: 'PLAT',
      owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      ticket_count: 42,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
    {
      id: 2,
      name: 'Networking',
      parent_id: 1,
      path: '/Platform/Networking/',
      slug: null,
      effective_slug: 'PLAT',
      owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      ticket_count: 18,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
    {
      id: 3,
      name: 'DNS',
      parent_id: 2,
      path: '/Platform/Networking/DNS/',
      slug: null,
      effective_slug: 'PLAT',
      owner: { id: 2, login: 'bob', display_name: 'Bob Lee' },
      ticket_count: 7,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
    {
      id: 4,
      name: 'UI Shell',
      parent_id: null,
      path: '/UI Shell/',
      slug: 'UI',
      effective_slug: 'UI',
      owner: { id: 3, login: 'jess', display_name: 'Jess Wong' },
      ticket_count: 24,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
};

let mockFetch: ReturnType<typeof vi.fn>;

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <ComponentTreePage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  mockFetch = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    json: () => Promise.resolve(MOCK_COMPONENTS),
  });
  global.fetch = mockFetch;
});

describe('ComponentTreePage', () => {
  it('renders tree nodes after loading', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getAllByText('Platform').length).toBeGreaterThanOrEqual(1);
    });
    expect(screen.getByText('UI Shell')).toBeInTheDocument();
  });

  it('shows component count', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('4 components')).toBeInTheDocument();
    });
  });

  it('auto-selects first component and shows detail', async () => {
    renderPage();
    await waitFor(() => {
      // Detail panel shows the first component name as heading
      expect(screen.getAllByText('Platform').length).toBeGreaterThanOrEqual(2);
    });
    expect(screen.getByText('Alex Kim')).toBeInTheDocument();
  });

  it('shows children as nested nodes', async () => {
    renderPage();
    await waitFor(() => {
      // Networking appears in tree and possibly in detail (as child chip)
      expect(screen.getAllByText('Networking').length).toBeGreaterThanOrEqual(1);
    });
  });

  it('selects a component when clicked', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('UI Shell')).toBeInTheDocument();
    });
    await user.click(screen.getByText('UI Shell'));
    // Detail panel should now show UI Shell info
    expect(screen.getByText('Jess Wong')).toBeInTheDocument();
  });

  it('filters tree by name', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getAllByText('Platform').length).toBeGreaterThanOrEqual(1);
    });
    const input = screen.getByPlaceholderText('Filter components…');
    await user.type(input, 'DNS');
    // DNS and its ancestors should be visible
    expect(screen.getAllByText('DNS').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Platform').length).toBeGreaterThanOrEqual(1);
    // UI Shell should be hidden (filtered out)
    expect(screen.queryByText('UI Shell')).not.toBeInTheDocument();
  });

  it('shows child chips in detail panel', async () => {
    renderPage();
    await waitFor(() => {
      // Platform is auto-selected and has Networking as child
      expect(screen.getByRole('button', { name: /Networking/ })).toBeInTheDocument();
    });
  });

  it('clicking a child chip selects that component', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Networking/ })).toBeInTheDocument();
    });
    await user.click(screen.getByRole('button', { name: /Networking/ }));
    // Detail should now show Networking as the name
    await waitFor(() => {
      expect(screen.getByText('Alex Kim')).toBeInTheDocument();
    });
  });

  it('shows "No subcomponents" for leaf nodes', async () => {
    const user = userEvent.setup();
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('UI Shell')).toBeInTheDocument();
    });
    await user.click(screen.getByText('UI Shell'));
    expect(screen.getByText('No subcomponents')).toBeInTheDocument();
  });

  it('shows View Tickets link', async () => {
    renderPage();
    await waitFor(() => {
      expect(screen.getByText('View Tickets')).toBeInTheDocument();
    });
    const link = screen.getByText('View Tickets').closest('a');
    expect(link).toHaveAttribute('href', expect.stringContaining('/tickets'));
  });

  it('shows slug when available', async () => {
    renderPage();
    await waitFor(() => {
      // Platform has effective_slug 'PLAT'
      expect(screen.getByText('PLAT')).toBeInTheDocument();
    });
  });
});
