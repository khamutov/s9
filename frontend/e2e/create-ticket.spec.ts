import { test, expect } from './fixtures/test-fixtures';

const MOCK_COMPONENTS = {
  items: [
    {
      id: 1,
      name: 'Platform',
      path: 'Platform',
      slug: 'PLAT',
      effective_slug: 'PLAT',
      owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
      ticket_count: 5,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
};

const MOCK_MILESTONES = {
  items: [
    {
      id: 1,
      name: 'v1.0 Launch',
      status: 'open',
      stats: { total: 10, new: 5, in_progress: 3, verify: 1, done: 1, estimated_hours: 40, remaining_hours: 20 },
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
};

const MOCK_USERS = {
  items: [
    { id: 1, login: 'alex', display_name: 'Alex Kim', email: 'alex@s9.dev', role: 'admin', is_active: true, has_password: true, has_oidc: false, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' },
    { id: 2, login: 'maria', display_name: 'Maria Chen', email: 'maria@s9.dev', role: 'user', is_active: true, has_password: true, has_oidc: false, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' },
  ],
};

const CREATED_TICKET = {
  id: 42,
  title: 'Fix login bug',
  slug: 'PLAT-42',
  type: 'bug',
  status: 'new',
  priority: 'P1',
  owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  component: { id: 1, name: 'Platform', path: 'Platform', slug: 'PLAT' },
  created_by: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  cc: [],
  milestones: [],
  comment_count: 0,
  estimation_hours: null,
  estimation_display: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
};

async function setupMocks(mockApi: Parameters<Parameters<typeof test>[2]>[0]['mockApi']) {
  await mockApi.loginAs();
  await mockApi.get('/api/components', MOCK_COMPONENTS);
  await mockApi.get('/api/milestones?status=open', MOCK_MILESTONES);
  await mockApi.get('/api/users', MOCK_USERS);
}

test.describe('Create Ticket Page', () => {
  test('renders form with all sections', async ({ page, mockApi }) => {
    await setupMocks(mockApi);
    await page.goto('/tickets/new');

    // Basic info section
    await expect(page.getByText('Basic Info')).toBeVisible();
    await expect(page.getByLabel(/title/i)).toBeVisible();
    await expect(page.getByLabel('Markdown editor')).toBeVisible();

    // Metadata section
    await expect(page.getByText('Metadata')).toBeVisible();
    await expect(page.getByLabel(/type/i)).toBeVisible();
    await expect(page.getByLabel(/priority/i)).toBeVisible();
    await expect(page.getByLabel(/component/i)).toBeVisible();
    await expect(page.getByLabel(/owner/i)).toBeVisible();
    await expect(page.getByLabel(/milestone/i)).toBeVisible();

    // Action buttons
    await expect(page.getByRole('button', { name: /create ticket/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /cancel/i })).toBeVisible();
  });

  test('populates select options from API', async ({ page, mockApi }) => {
    await setupMocks(mockApi);
    await page.goto('/tickets/new');

    // Components loaded
    await expect(page.getByRole('option', { name: 'Platform' })).toBeAttached();

    // Users loaded
    await expect(page.getByRole('option', { name: 'Alex Kim' })).toBeAttached();
    await expect(page.getByRole('option', { name: 'Maria Chen' })).toBeAttached();

    // Milestones loaded
    await expect(page.getByRole('option', { name: 'v1.0 Launch' })).toBeAttached();
  });

  test('shows validation errors for empty required fields', async ({ page, mockApi }) => {
    await setupMocks(mockApi);
    await page.goto('/tickets/new');

    await page.getByRole('button', { name: /create ticket/i }).click();

    await expect(page.getByText('Title is required')).toBeVisible();
    await expect(page.getByText('Component is required')).toBeVisible();
    await expect(page.getByText('Owner is required')).toBeVisible();
  });

  test('submits form and navigates to created ticket', async ({ page, mockApi }) => {
    await setupMocks(mockApi);
    await mockApi.post('/api/tickets', CREATED_TICKET);

    // Mock ticket detail page data
    await mockApi.get('/api/tickets/42', CREATED_TICKET);
    await mockApi.get('/api/tickets/42/comments', { items: [] });

    await page.goto('/tickets/new');

    // Fill form
    await page.getByLabel(/title/i).fill('Fix login bug');
    await page.getByLabel(/type/i).selectOption('bug');
    await page.getByLabel(/priority/i).selectOption('P1');
    await page.getByLabel(/component/i).selectOption('1');
    await page.getByLabel(/owner/i).selectOption('1');

    await page.getByRole('button', { name: /create ticket/i }).click();

    // Should navigate to ticket detail
    await expect(page).toHaveURL(/\/tickets\/42/);
  });

  test('cancel navigates back to ticket list', async ({ page, mockApi }) => {
    await setupMocks(mockApi);
    // Mock ticket list for navigation target
    await mockApi.get('/api/tickets', { items: [], has_more: false });

    await page.goto('/tickets/new');
    await page.getByRole('button', { name: /cancel/i }).click();

    await expect(page).toHaveURL(/\/tickets$/);
  });

  test('clears validation error when field is corrected', async ({ page, mockApi }) => {
    await setupMocks(mockApi);
    await page.goto('/tickets/new');

    // Trigger errors
    await page.getByRole('button', { name: /create ticket/i }).click();
    await expect(page.getByText('Title is required')).toBeVisible();

    // Fix title
    await page.getByLabel(/title/i).fill('Some title');
    await expect(page.getByText('Title is required')).not.toBeVisible();
  });
});
