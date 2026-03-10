import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER } from './fixtures/mock-data';

const MOCK_TICKET = {
  id: 42,
  slug: 'S9-42',
  type: 'bug',
  title: 'Markdown renderer test',
  status: 'new',
  priority: 'P3',
  owner: { id: 1, login: 'testuser', display_name: 'Test User' },
  created_by: { id: 1, login: 'testuser', display_name: 'Test User' },
  component: { id: 1, name: 'Core', path: '/Core/' },
  cc: [],
  milestones: [],
  comment_count: 2,
  created_at: '2026-03-01T00:00:00Z',
  updated_at: '2026-03-01T00:00:00Z',
};

const MOCK_COMMENTS = {
  items: [
    {
      id: 1,
      ticket_id: 42,
      number: 0,
      body: 'This is **bold** and *italic* with `inline code`.\n\nSee #99 and #MAP-5 for context. Also @alice mentioned it.',
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-01T00:00:00Z',
      updated_at: '2026-03-01T00:00:00Z',
    },
    {
      id: 2,
      ticket_id: 42,
      number: 1,
      body: 'Related to comment#1 in this ticket.',
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-01T01:00:00Z',
      updated_at: '2026-03-01T01:00:00Z',
    },
  ],
};

test.describe('Markdown Renderer', () => {
  test.beforeEach(async ({ mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', MOCK_TICKET);
    await mockApi.get('/api/tickets/42/comments', MOCK_COMMENTS);
  });

  test('renders markdown formatting in description', async ({ page }) => {
    await page.goto('/tickets/42');
    // Bold text rendered
    const bold = page.locator('strong', { hasText: 'bold' });
    await expect(bold).toBeVisible();
    // Italic text rendered
    const italic = page.locator('em', { hasText: 'italic' });
    await expect(italic).toBeVisible();
    // Inline code rendered
    const code = page.locator('code', { hasText: 'inline code' });
    await expect(code).toBeVisible();
  });

  test('renders #99 as clickable ticket link', async ({ page }) => {
    await page.goto('/tickets/42');
    const link = page.getByRole('link', { name: '#99' });
    await expect(link).toBeVisible();
    await expect(link).toHaveAttribute('href', '/tickets/99');
  });

  test('renders #MAP-5 as clickable ticket link', async ({ page }) => {
    await page.goto('/tickets/42');
    const link = page.getByRole('link', { name: '#MAP-5' });
    await expect(link).toBeVisible();
    await expect(link).toHaveAttribute('href', '/tickets/5');
  });

  test('renders @alice as mention', async ({ page }) => {
    await page.goto('/tickets/42');
    await expect(page.getByText('@alice')).toBeVisible();
  });

  test('renders comment#1 as anchor link in activity', async ({ page }) => {
    await page.goto('/tickets/42');
    const link = page.getByRole('link', { name: 'comment#1' });
    await expect(link).toBeVisible();
    await expect(link).toHaveAttribute('href', '#comment-1');
  });
});
