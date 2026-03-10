import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER } from './fixtures/mock-data';

const MOCK_TICKET = {
  id: 42,
  slug: 'S9-42',
  type: 'bug',
  title: 'Crash on startup when config is missing',
  status: 'in_progress',
  priority: 'P1',
  owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  component: { id: 5, name: 'DNS', path: '/Platform/Networking/DNS/' },
  created_by: { id: 2, login: 'maria', display_name: 'Maria Chen' },
  cc: [{ id: 3, login: 'bob', display_name: 'Bob Lee' }],
  milestones: [{ id: 1, name: 'v2.4' }],
  estimation_hours: 16,
  estimation_display: '2d',
  comment_count: 2,
  created_at: '2026-03-04T10:00:00.000Z',
  updated_at: '2026-03-06T14:30:00.000Z',
};

const MOCK_COMMENTS = {
  items: [
    {
      id: 1,
      ticket_id: 42,
      number: 0,
      author: { id: 2, login: 'maria', display_name: 'Maria Chen' },
      body: 'The application panics on startup when config is missing.',
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-04T10:00:00.000Z',
      updated_at: '2026-03-04T10:00:00.000Z',
    },
    {
      id: 2,
      ticket_id: 42,
      number: 1,
      author: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      body: 'I can reproduce this consistently.',
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-05T10:00:00.000Z',
      updated_at: '2026-03-05T10:00:00.000Z',
    },
  ],
};

test.describe('Ticket Detail Page', () => {
  test.beforeEach(async ({ mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', MOCK_TICKET);
    await mockApi.get('/api/tickets/42/comments', MOCK_COMMENTS);
  });

  test('displays ticket title and slug', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(
      page.getByRole('heading', { name: 'Crash on startup when config' }).first(),
    ).toBeVisible();
    await expect(page.getByText('S9-42').first()).toBeVisible();
  });

  test('shows status, priority, and type badges', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('In Progress').first()).toBeVisible();
    await expect(page.getByText('P1').first()).toBeVisible();
    await expect(page.getByText('Bug').first()).toBeVisible();
  });

  test('renders metadata panel with owner and component', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('Alex Kim').first()).toBeVisible();
    await expect(page.getByText('/Platform/Networking/DNS/')).toBeVisible();
  });

  test('shows description from comment #0', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(
      page.getByText('The application panics on startup when config is missing.'),
    ).toBeVisible();
  });

  test('shows activity comments', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(
      page.getByText('I can reproduce this consistently.'),
    ).toBeVisible();
    // Activity header with count
    await expect(page.getByRole('heading', { level: 2, name: 'Activity' })).toBeVisible();
  });

  test('has breadcrumb link back to ticket list', async ({ page }) => {
    await page.goto('/tickets/42');
    const ticketsLink = page.getByRole('link', { name: 'Tickets', exact: true }).first();
    await expect(ticketsLink).toBeVisible();
    await expect(ticketsLink).toHaveAttribute('href', '/tickets');
  });

  test('shows milestone and estimation in metadata', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('v2.4')).toBeVisible();
    await expect(page.getByText('2d')).toBeVisible();
  });
});
