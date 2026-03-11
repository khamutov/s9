import { test, expect } from './fixtures/test-fixtures';

const SEARCH_RESULTS = {
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
  ],
  total: 1,
  page: 1,
  page_size: 8,
};

test.describe('CommandBar quick-jump search', () => {
  test.beforeEach(async ({ mockApi, page }) => {
    await mockApi.loginAs();
    // Mock milestones for the page we navigate to (CommandBar is global)
    await mockApi.get('/api/milestones', { items: [] });
    // Mock ticket search endpoint used by CommandBar
    await page.route('**/api/tickets?*', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(SEARCH_RESULTS),
      });
    });
  });

  test('renders search input', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByPlaceholder('Search or jump to...')).toBeVisible();
  });

  test('shows search results when typing', async ({ page }) => {
    await page.goto('/milestones');
    await page.getByPlaceholder('Search or jump to...').fill('crash');
    await expect(page.getByText('PLAT-42')).toBeVisible();
    await expect(page.getByText('Crash on startup when config is missing')).toBeVisible();
  });

  test('navigates to ticket on result click', async ({ page, mockApi }) => {
    // Also mock the ticket detail page so navigation succeeds
    await mockApi.get('/api/tickets/42', SEARCH_RESULTS.items[0]);
    await mockApi.get('/api/tickets/42/comments', { items: [] });

    await page.goto('/milestones');
    await page.getByPlaceholder('Search or jump to...').fill('crash');
    // Use mousedown on the result item (matches the onMouseDown handler)
    await page.getByRole('option', { name: /Crash on startup/ }).dispatchEvent('mousedown');
    await expect(page).toHaveURL(/\/tickets\/42/);
  });

  test('closes dropdown on Escape', async ({ page }) => {
    await page.goto('/milestones');
    const input = page.getByPlaceholder('Search or jump to...');
    await input.fill('crash');
    await expect(page.getByText('PLAT-42')).toBeVisible();
    await input.press('Escape');
    await expect(page.getByText('PLAT-42')).not.toBeVisible();
  });

  test('keyboard navigation with ArrowDown and Enter', async ({ page, mockApi }) => {
    await mockApi.get('/api/tickets/42', SEARCH_RESULTS.items[0]);
    await mockApi.get('/api/tickets/42/comments', { items: [] });

    await page.goto('/milestones');
    const input = page.getByPlaceholder('Search or jump to...');
    await input.fill('crash');
    await expect(page.getByText('PLAT-42')).toBeVisible();
    await input.press('ArrowDown');
    await input.press('Enter');
    await expect(page).toHaveURL(/\/tickets\/42/);
  });
});
