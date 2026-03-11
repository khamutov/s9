import { test, expect } from './fixtures/test-fixtures';
import { TEST_ADMIN } from './fixtures/mock-data';

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
      stats: { total: 0, new: 0, in_progress: 0, verify: 0, done: 0, estimated_hours: 0, remaining_hours: 0 },
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

  test('hides CRUD buttons for non-admin users', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByText('v1.0 Launch')).toBeVisible();
    // Default test user is non-admin — no Create/Edit/Delete buttons
    await expect(page.getByRole('button', { name: /Create Milestone/ })).not.toBeVisible();
    await expect(page.getByRole('button', { name: 'Edit' }).first()).not.toBeVisible();
  });
});

test.describe('Milestone CRUD (admin)', () => {
  test.beforeEach(async ({ mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.get('/api/milestones', MILESTONES);
  });

  test('shows Create Milestone button for admin', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByRole('button', { name: /Create Milestone/ })).toBeVisible();
  });

  test('shows Edit and Delete buttons on cards', async ({ page }) => {
    await page.goto('/milestones');
    await expect(page.getByRole('button', { name: 'Edit' }).first()).toBeVisible();
    await expect(page.getByRole('button', { name: 'Delete' }).first()).toBeVisible();
  });

  test('opens create modal and submits', async ({ page, mockApi }) => {
    const created = {
      id: 3,
      name: 'v2.0',
      description: 'Next release',
      due_date: null,
      status: 'open',
      stats: { total: 0, new: 0, in_progress: 0, verify: 0, done: 0, estimated_hours: 0, remaining_hours: 0 },
      created_at: '2026-03-11T00:00:00Z',
      updated_at: '2026-03-11T00:00:00Z',
    };
    await mockApi.post('/api/milestones', created, 201);

    await page.goto('/milestones');
    await page.getByRole('button', { name: /Create Milestone/ }).click();

    const dialog = page.getByRole('dialog', { name: 'Create Milestone' });
    await expect(dialog).toBeVisible();

    await dialog.getByLabel(/Name/).fill('v2.0');
    await dialog.getByLabel(/Description/).fill('Next release');
    await dialog.getByRole('button', { name: /Create Milestone/ }).click();

    // Modal should close after submission
    await expect(dialog).not.toBeVisible();
  });

  test('opens edit modal with pre-filled data', async ({ page }) => {
    await page.goto('/milestones');
    await page.getByRole('button', { name: 'Edit' }).first().click();

    const dialog = page.getByRole('dialog', { name: 'Edit Milestone' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByLabel(/Name/)).toHaveValue('v1.0 Launch');
  });

  test('opens delete modal and shows warning for milestone with tickets', async ({ page }) => {
    await page.goto('/milestones');
    // Click Delete on first milestone (has 10 tickets)
    await page.getByRole('button', { name: 'Delete' }).first().click();

    const dialog = page.getByRole('dialog', { name: 'Delete Milestone' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/10 assigned tickets/)).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Delete' })).toBeDisabled();
  });

  test('allows delete of milestone with no tickets', async ({ page, mockApi }) => {
    await mockApi.route('DELETE', '/api/milestones/2', {}, 204);

    await page.goto('/milestones');
    // Click Delete on second milestone (Backlog, 0 tickets)
    await page.getByRole('button', { name: 'Delete' }).nth(1).click();

    const dialog = page.getByRole('dialog', { name: 'Delete Milestone' });
    await expect(dialog.getByRole('button', { name: 'Delete' })).toBeEnabled();
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(dialog).not.toBeVisible();
  });

  test('closes modal on Cancel', async ({ page }) => {
    await page.goto('/milestones');
    await page.getByRole('button', { name: /Create Milestone/ }).click();

    const dialog = page.getByRole('dialog', { name: 'Create Milestone' });
    await expect(dialog).toBeVisible();

    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).not.toBeVisible();
  });
});
