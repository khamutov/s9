import { test, expect } from './fixtures/test-fixtures';

const MOCK_COMPONENTS = {
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
      name: 'Auth',
      parent_id: 1,
      path: '/Platform/Auth/',
      slug: null,
      effective_slug: 'PLAT',
      owner: { id: 3, login: 'maria', display_name: 'Maria Chen' },
      ticket_count: 12,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
};

test.describe('Component Tree Page', () => {
  test.beforeEach(async ({ mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/components', MOCK_COMPONENTS);
  });

  test('renders component tree with nodes', async ({ page }) => {
    await page.goto('/components');
    // Platform appears in both tree and detail panel
    await expect(page.getByText('Platform').first()).toBeVisible();
    await expect(page.getByText('4 components')).toBeVisible();
  });

  test('shows detail panel for selected component', async ({ page }) => {
    await page.goto('/components');
    // Wait for tree to load and auto-select first component
    await expect(page.getByText('View Tickets')).toBeVisible();
    await expect(page.getByText('Alex Kim').first()).toBeVisible();
  });

  test('clicking a tree node shows its details', async ({ page }) => {
    await page.goto('/components');
    // Click Auth in the tree (use treeitem role to target tree node)
    await page.getByRole('treeitem', { name: /Auth/ }).click();
    await expect(page.getByText('Maria Chen')).toBeVisible();
  });

  test('filter narrows visible tree nodes', async ({ page }) => {
    await page.goto('/components');
    await page.getByPlaceholder('Filter components…').fill('DNS');
    // DNS should be visible (use first() since it may appear in detail too)
    await expect(page.getByText('DNS').first()).toBeVisible();
    await expect(page.getByText('Platform').first()).toBeVisible();
  });

  test('shows nested children in tree', async ({ page }) => {
    await page.goto('/components');
    // Networking is a child of Platform (auto-expanded root)
    await expect(page.getByText('Networking').first()).toBeVisible();
    await expect(page.getByText('Auth').first()).toBeVisible();
  });

  test('View Tickets link navigates correctly', async ({ page }) => {
    await page.goto('/components');
    const link = page.getByText('View Tickets');
    await expect(link).toHaveAttribute('href', /\/tickets/);
  });
});
