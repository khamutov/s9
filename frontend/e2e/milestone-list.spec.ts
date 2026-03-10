import { test, expect } from './fixtures/test-fixtures';

const MILESTONES = {
  items: [
    {
      id: 1,
      name: 'v1.0 Launch',
      description: 'Core platform release with ticket management.',
      due_date: '2026-04-01',
      status: 'open',
      stats: { total: 10, new: 1, in_progress: 3, verify: 1, done: 5, estimated_hours: 40, remaining_hours: 20 },
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-03-01T00:00:00Z',
    },
    {
      id: 2,
      name: 'Backlog',
      description: null,
      due_date: null,
      status: 'open',
      stats: { total: 4, new: 3, in_progress: 1, verify: 0, done: 0, estimated_hours: 0, remaining_hours: 0 },
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-02-01T00:00:00Z',
    },
  ],
};

test.describe('Milestone list page', () => {
  test.beforeEach(async ({ mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/milestones', MILESTONES);
  });

  test('renders milestone cards', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByText('v1.0 Launch')).toBeVisible();
    await expect(page.getByText('Backlog')).toBeVisible();
  });

  test('shows page title and count', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByRole('heading', { name: /Milestones/ }).first()).toBeVisible();
    // Count shown next to title
    await expect(page.getByRole('heading', { name: /Milestones 2/ }).first()).toBeVisible();
  });

  test('shows progress percentage', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByText('50%')).toBeVisible();
    await expect(page.getByText('0%', { exact: true })).toBeVisible();
  });

  test('shows due date and dash for missing', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByText(/Due Apr/)).toBeVisible();
    await expect(page.getByText('\u2014')).toBeVisible();
  });

  test('filters milestones by name', async ({ page }) => {
    await page.goto('/milestones');
    await page.getByPlaceholder('Filter milestones...').fill('Backlog');
    await expect(page.getByText('Backlog')).toBeVisible();
    await expect(page.getByText('v1.0 Launch')).not.toBeVisible();
  });

  test('View Tickets link navigates to filtered ticket list', async ({ page }) => {
    await page.goto('/milestones');
    const link = page.getByRole('link', { name: 'View Tickets' }).first();
    await expect(link).toHaveAttribute('href', '/tickets?q=milestone:v1.0%20Launch');
  });
});
