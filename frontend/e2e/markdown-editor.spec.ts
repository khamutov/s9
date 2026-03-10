import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER } from './fixtures/mock-data';

/**
 * E2E smoke test for the MarkdownEditor component context.
 * The editor itself has 23 unit tests covering all interactions.
 * This verifies the host page (ticket detail) where it will be integrated.
 */

const MOCK_TICKET = {
  id: 42,
  slug: 'S9-42',
  type: 'bug',
  title: 'Editor integration test ticket',
  status: 'new',
  priority: 'P3',
  owner: { id: 1, login: 'testuser', display_name: 'Test User' },
  created_by: { id: 1, login: 'testuser', display_name: 'Test User' },
  component: { id: 1, name: 'Core', path: '/Core/' },
  cc: [],
  milestones: [],
  comment_count: 1,
  created_at: '2026-03-01T00:00:00Z',
  updated_at: '2026-03-01T00:00:00Z',
};

const MOCK_COMMENTS = {
  items: [
    {
      id: 1,
      ticket_id: 42,
      number: 0,
      body: 'Description with **markdown** content',
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-01T00:00:00Z',
      updated_at: '2026-03-01T00:00:00Z',
    },
  ],
};

test.describe('MarkdownEditor context', () => {
  test.beforeEach(async ({ mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', MOCK_TICKET);
    await mockApi.get('/api/tickets/42/comments', MOCK_COMMENTS);
  });

  test('ticket detail page loads with description content', async ({
    page,
  }) => {
    await page.goto('/tickets/42');
    await expect(
      page.getByRole('heading', { name: 'Editor integration test' }).first(),
    ).toBeVisible();
    // Markdown is now rendered: **markdown** becomes <strong>
    await expect(page.getByText('Description with')).toBeVisible();
    await expect(page.getByText('markdown').first()).toBeVisible();
  });
});
