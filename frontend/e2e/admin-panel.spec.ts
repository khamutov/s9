import { test, expect } from './fixtures/test-fixtures';
import { TEST_ADMIN, TEST_USER } from './fixtures/mock-data';

const MOCK_USERS = {
  items: [
    {
      id: 1,
      login: 'alice',
      display_name: 'Alice Admin',
      email: 'alice@s9.dev',
      role: 'admin',
      is_active: true,
      has_password: true,
      has_oidc: false,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-03-01T00:00:00Z',
    },
    {
      id: 2,
      login: 'bob',
      display_name: 'Bob User',
      email: 'bob@s9.dev',
      role: 'user',
      is_active: true,
      has_password: true,
      has_oidc: true,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-03-01T00:00:00Z',
    },
  ],
};

const MOCK_COMPONENTS = {
  items: [
    {
      id: 1,
      name: 'Platform',
      parent_id: null,
      path: 'Platform',
      slug: 'PLAT',
      effective_slug: 'PLAT',
      owner: { id: 1, login: 'alice', display_name: 'Alice Admin' },
      ticket_count: 5,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-03-01T00:00:00Z',
    },
  ],
};

test.describe('Admin Panel', () => {
  test('shows navigation cards for admin user', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await page.goto('/admin');
    // Card titles (distinct from sidebar nav items by checking the card area)
    await expect(page.getByText('Manage user accounts, roles, and access')).toBeVisible();
    await expect(page.getByText('Manage the component tree, slugs, and ownership')).toBeVisible();
    await expect(page.getByText('System configuration and preferences')).toBeVisible();
  });

  test('shows access denied for non-admin', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await page.goto('/admin');
    await expect(page.getByText(/administrator privileges/)).toBeVisible();
  });

  test('navigates to user management', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.get('/api/users', MOCK_USERS);
    await page.goto('/admin');
    // Click the Users card link (not sidebar)
    await page.getByText('Manage user accounts, roles, and access').click();
    await expect(page).toHaveURL(/\/admin\/users/);
  });
});

test.describe('User Management', () => {
  test('displays user table', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.get('/api/users', MOCK_USERS);
    await page.goto('/admin/users');
    await expect(page.getByText('Alice Admin')).toBeVisible();
    await expect(page.getByText('Bob User')).toBeVisible();
    await expect(page.getByText('bob@s9.dev')).toBeVisible();
  });

  test('opens create user modal', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.get('/api/users', MOCK_USERS);
    await page.goto('/admin/users');
    await page.getByText('Add User').click();
    await expect(page.getByRole('heading', { name: 'Create User' })).toBeVisible();
  });

  test('opens edit user modal', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.get('/api/users', MOCK_USERS);
    await page.goto('/admin/users');
    await page.getByText('Edit').first().click();
    await expect(page.getByRole('heading', { name: 'Edit User' })).toBeVisible();
  });
});

test.describe('Component Management', () => {
  test('displays component table', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.get('/api/components', MOCK_COMPONENTS);
    await mockApi.get('/api/users', MOCK_USERS);
    await page.goto('/admin/components');
    await expect(page.getByText('Alice Admin').first()).toBeVisible();
    await expect(page.getByText('PLAT').first()).toBeVisible();
  });

  test('opens create component modal', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await mockApi.get('/api/components', MOCK_COMPONENTS);
    await mockApi.get('/api/users', MOCK_USERS);
    await page.goto('/admin/components');
    await page.getByText('Add Component').click();
    await expect(page.getByRole('heading', { name: 'Create Component' })).toBeVisible();
  });
});

test.describe('System Settings', () => {
  test('displays settings sections', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_ADMIN);
    await page.goto('/admin/settings');
    await expect(page.getByText('General')).toBeVisible();
    await expect(page.getByText('Authentication')).toBeVisible();
    await expect(page.getByText('Password Auth')).toBeVisible();
  });

  test('shows access denied for non-admin', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await page.goto('/admin/settings');
    await expect(page.getByText(/administrator privileges/)).toBeVisible();
  });
});
