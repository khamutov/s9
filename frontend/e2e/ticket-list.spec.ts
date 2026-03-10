import { test, expect } from './fixtures/test-fixtures';
import type { Ticket, CursorPage } from '../src/api/types';

const makeTicket = (overrides: Partial<Ticket> = {}): Ticket => ({
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
  created_at: '2026-03-10T10:00:00Z',
  updated_at: '2026-03-10T10:00:00Z',
  ...overrides,
});

const MOCK_TICKETS: CursorPage<Ticket> = {
  items: [
    makeTicket({
      id: 1,
      title: 'Crash on startup',
      slug: 'AUTH-1',
      status: 'new',
      priority: 'P1',
    }),
    makeTicket({
      id: 2,
      title: 'Add bulk edit',
      slug: 'S9-2',
      status: 'in_progress',
      priority: 'P2',
      owner: { id: 2, login: 'maria', display_name: 'Maria Chen' },
      component: { id: 2, name: 'Tickets', path: 'Tickets' },
    }),
    makeTicket({
      id: 3,
      title: 'Fix search filter',
      slug: 'S9-3',
      status: 'done',
      priority: 'P3',
      component: { id: 3, name: 'Search', path: 'Search' },
    }),
  ],
  has_more: false,
};

test.describe('Ticket List Page', () => {
  test('displays ticket table with data', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', MOCK_TICKETS);

    await page.goto('/tickets');

    // Table rows
    await expect(page.getByText('Crash on startup')).toBeVisible();
    await expect(page.getByText('Add bulk edit')).toBeVisible();
    await expect(page.getByText('Fix search filter')).toBeVisible();

    // Slugs
    await expect(page.getByText('AUTH-1')).toBeVisible();
    await expect(page.getByText('S9-2')).toBeVisible();

    // Owners
    await expect(page.getByText('Alex Kim').first()).toBeVisible();
    await expect(page.getByText('Maria Chen')).toBeVisible();

    // Components in table
    const table = page.locator('table');
    await expect(table.getByText('Auth', { exact: true })).toBeVisible();
    await expect(table.getByText('Tickets', { exact: true })).toBeVisible();
  });

  test('shows empty state when no tickets', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', { items: [], has_more: false });

    await page.goto('/tickets');

    await expect(page.getByText('No tickets found.')).toBeVisible();
  });

  test('navigates to ticket detail on row click', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', {
      items: [makeTicket({ id: 42, title: 'Clickable ticket' })],
      has_more: false,
    });

    await page.goto('/tickets');

    await page.getByText('Clickable ticket').click();
    await expect(page).toHaveURL(/\/tickets\/42/);
  });

  test('shows Create Ticket button linking to /tickets/new', async ({
    page,
    mockApi,
  }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', { items: [], has_more: false });

    await page.goto('/tickets');

    const createLink = page.getByRole('link', { name: /create ticket/i });
    await expect(createLink).toBeVisible();
    await expect(createLink).toHaveAttribute('href', '/tickets/new');
  });

  test('filter bar filters tickets via query param', async ({ page, mockApi }) => {
    await mockApi.loginAs();

    // Default: return all tickets
    await mockApi.get('/api/tickets', MOCK_TICKETS);

    // Filtered: return only the P1 ticket when q param is present
    const filteredResults: CursorPage<Ticket> = {
      items: [MOCK_TICKETS.items[0]],
      has_more: false,
    };

    await page.goto('/tickets');
    await expect(page.getByText('Crash on startup')).toBeVisible();
    await expect(page.getByText('Add bulk edit')).toBeVisible();

    // Mock the filtered request before typing
    // Route with query params will match the glob
    await page.route('**/api/tickets?*', async (route) => {
      const url = new URL(route.request().url());
      if (url.searchParams.get('q')?.includes('priority:P1')) {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(filteredResults),
        });
      } else {
        await route.fallback();
      }
    });

    const filterInput = page.getByRole('textbox', { name: /filter tickets/i });
    await filterInput.fill('priority:P1');

    // Should show only the P1 ticket
    await expect(page.getByText('Crash on startup')).toBeVisible();
    await expect(page.getByText('Add bulk edit')).not.toBeVisible();
  });

  test('filter bar shows autocomplete on focus', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', { items: [], has_more: false });

    await page.goto('/tickets');

    const filterInput = page.getByRole('textbox', { name: /filter tickets/i });
    await filterInput.click();

    // Should show filter key suggestions
    await expect(page.getByText('status:')).toBeVisible();
    await expect(page.getByText('priority:')).toBeVisible();
    await expect(page.getByText('owner:')).toBeVisible();
  });

  test('filter bar supports "/" keyboard shortcut to focus', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', { items: [], has_more: false });

    await page.goto('/tickets');

    const filterInput = page.getByRole('textbox', { name: /filter tickets/i });
    await expect(filterInput).not.toBeFocused();

    // Press "/" to focus
    await page.keyboard.press('/');
    await expect(filterInput).toBeFocused();
  });

  test('renders table headers', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', MOCK_TICKETS);

    await page.goto('/tickets');

    // Wait for data to load
    await expect(page.getByText('Crash on startup')).toBeVisible();

    // Table headers
    await expect(page.getByRole('columnheader', { name: 'ID' })).toBeVisible();
    await expect(page.getByRole('columnheader', { name: 'Title' })).toBeVisible();
    await expect(page.getByRole('columnheader', { name: 'Status' })).toBeVisible();
    await expect(page.getByRole('columnheader', { name: 'Pri' })).toBeVisible();
    await expect(page.getByRole('columnheader', { name: 'Owner' })).toBeVisible();
  });
});
