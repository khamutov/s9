import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER, TEST_ADMIN } from './fixtures/mock-data';

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
    await mockApi.get('/api/milestones?status=open', {
      items: [{ id: 1, name: 'v2.4', status: 'open', stats: { total: 5, new: 2, in_progress: 1, verify: 1, done: 1, estimated_hours: 20, remaining_hours: 10 }, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' }],
    });
  });

  test('displays ticket title and slug', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(
      page.getByRole('heading', { level: 1 }).first(),
    ).toBeVisible();
    await expect(page.getByText('Crash on startup when config is missing').first()).toBeVisible();
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

  test('inline-edit status via dropdown', async ({ page, mockApi }) => {
    const updatedTicket = { ...MOCK_TICKET, status: 'done' };
    await mockApi.patch('/api/tickets/42', updatedTicket);
    await page.goto('/tickets/42');

    // Click the status trigger in the metadata panel
    const statusBtn = page.getByRole('button', { name: 'Status' });
    await statusBtn.click();

    // Select 'Done' from the dropdown
    await page.getByRole('option', { name: 'Done' }).click();

    // Dropdown should close
    await expect(page.getByRole('listbox')).not.toBeVisible();
  });

  test('inline-edit title via text input', async ({ page, mockApi }) => {
    const updatedTicket = { ...MOCK_TICKET, title: 'Updated title' };
    await mockApi.patch('/api/tickets/42', updatedTicket);
    await page.goto('/tickets/42');

    // Click the title to enter edit mode
    await page.getByRole('button', { name: 'Edit Title' }).click();

    const input = page.getByRole('textbox', { name: 'Title' });
    await input.fill('Updated title');
    await input.press('Enter');

    // Should return to display mode
    await expect(page.getByRole('textbox', { name: 'Title' })).not.toBeVisible();
  });

  test('inline-edit owner via dropdown', async ({ page, mockApi }) => {
    const updatedTicket = {
      ...MOCK_TICKET,
      owner: { id: 2, login: 'maria', display_name: 'Maria Chen' },
    };
    await mockApi.get('/api/users/compact', {
      items: [
        { id: 1, login: 'alex', display_name: 'Alex Kim' },
        { id: 2, login: 'maria', display_name: 'Maria Chen' },
        { id: 3, login: 'bob', display_name: 'Bob Lee' },
      ],
    });
    await mockApi.patch('/api/tickets/42', updatedTicket);
    await page.goto('/tickets/42');

    // Click the owner trigger
    await page.getByRole('button', { name: 'Owner' }).click();

    // Select Maria Chen from dropdown
    await page.getByRole('option', { name: /Maria Chen/ }).click();

    // Dropdown should close
    await expect(page.getByRole('listbox')).not.toBeVisible();
  });

  test('inline-edit description', async ({ page, mockApi }) => {
    // Login as admin to have edit permission on any comment
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.patch('/api/tickets/42/comments/0', {
      ...MOCK_COMMENTS.items[0],
      body: 'Updated description',
      edit_count: 1,
    });
    await page.goto('/tickets/42');

    // Hover over description card to reveal the Edit button
    await page.getByText('The application panics on startup').hover();
    // Click Edit on description
    await page.getByRole('button', { name: 'Edit description' }).click();

    // Editor should appear
    const editor = page.locator('textarea').first();
    await editor.fill('Updated description');

    // Click Save
    await page.getByRole('button', { name: 'Save' }).click();

    // Editor should close
    await expect(page.getByRole('button', { name: 'Save' })).not.toBeVisible();
  });

  test('inline-edit estimation via text input', async ({ page, mockApi }) => {
    const updatedTicket = { ...MOCK_TICKET, estimation_display: '4d' };
    await mockApi.patch('/api/tickets/42', updatedTicket);
    await page.goto('/tickets/42');

    // Click the estimate value to enter edit mode
    await page.getByRole('button', { name: 'Edit Estimate' }).click();

    // Fill in and save
    const input = page.getByRole('textbox', { name: 'Estimate' });
    await input.fill('4d');
    await input.press('Enter');

    // Should return to display mode
    await expect(page.getByRole('textbox', { name: 'Estimate' })).not.toBeVisible();
  });
});
