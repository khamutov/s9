import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER, TEST_ADMIN } from './fixtures/mock-data';

const MOCK_TICKET = {
  id: 42,
  slug: 'S9-42',
  type: 'bug',
  title: 'Crash on startup',
  status: 'in_progress',
  priority: 'P1',
  owner: { id: 1, login: 'testuser', display_name: 'Test User' },
  component: { id: 5, name: 'DNS', path: '/Platform/DNS/' },
  created_by: { id: 1, login: 'testuser', display_name: 'Test User' },
  cc: [],
  milestones: [],
  estimation_hours: null,
  estimation_display: null,
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
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      body: 'Description of the issue.',
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
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      body: 'I found the root cause.',
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-05T10:00:00.000Z',
      updated_at: '2026-03-05T10:00:00.000Z',
    },
    {
      id: 3,
      ticket_id: 42,
      number: 2,
      author: { id: 99, login: 'other', display_name: 'Other User' },
      body: 'Thanks for investigating.',
      attachments: [],
      edit_count: 1,
      edits: [],
      created_at: '2026-03-06T10:00:00.000Z',
      updated_at: '2026-03-06T12:00:00.000Z',
    },
  ],
};

test.describe('Comment Thread', () => {
  test.beforeEach(async ({ mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', MOCK_TICKET);
    await mockApi.get('/api/tickets/42/comments', MOCK_COMMENTS);
  });

  test('displays comment form with editor and submit button', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('Add comment')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Comment', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Comment', exact: true })).toBeDisabled();
  });

  test('shows edit button on own comments', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('I found the root cause.')).toBeVisible();
    // Own comment (#1) should have edit button
    await expect(page.getByRole('button', { name: 'Edit comment #1' })).toBeVisible();
    // Other user's comment (#2) should not have edit button
    await expect(page.getByRole('button', { name: 'Edit comment #2' })).not.toBeVisible();
  });

  test('shows edited tag on comments with edit_count > 0', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('Thanks for investigating.')).toBeVisible();
    await expect(page.getByText('edited')).toBeVisible();
  });

  test('opens edit form when clicking edit button', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('I found the root cause.')).toBeVisible();

    // Hover to reveal actions, then click edit
    await page.getByRole('button', { name: 'Edit comment #1' }).click({ force: true });

    // Save and Cancel buttons should appear
    await expect(page.getByRole('button', { name: 'Save' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeVisible();
  });

  test('cancels edit and returns to display mode', async ({ page }) => {
    await page.goto('/tickets/42');
    await page.getByRole('button', { name: 'Edit comment #1' }).click({ force: true });

    await page.getByRole('button', { name: 'Cancel' }).click();

    // Should see original text rendered again
    await expect(page.getByText('I found the root cause.')).toBeVisible();
    // Edit form should be gone
    await expect(page.getByRole('button', { name: 'Save' })).not.toBeVisible();
  });

  test('submits new comment via form', async ({ page, mockApi }) => {
    const newComment = {
      id: 4,
      ticket_id: 42,
      number: 3,
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      body: 'This is my new comment.',
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-10T10:00:00.000Z',
      updated_at: '2026-03-10T10:00:00.000Z',
    };
    await mockApi.post('/api/tickets/42/comments', newComment);
    await page.goto('/tickets/42');

    // Type in the editor textarea
    const textarea = page.locator('textarea').last();
    await textarea.fill('This is my new comment.');

    // Submit button should be enabled
    const submitBtn = page.getByRole('button', { name: 'Comment', exact: true });
    await expect(submitBtn).toBeEnabled();
    await submitBtn.click();
  });

  test('admin sees delete buttons on other users comments', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await page.goto('/tickets/42');

    await expect(page.getByText('Thanks for investigating.')).toBeVisible();
    // Admin should see delete button on comment #2 (another user's comment)
    await expect(page.getByRole('button', { name: 'Delete comment #2' })).toBeVisible();
  });
});
