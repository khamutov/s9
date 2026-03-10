import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER } from './fixtures/mock-data';

test('successful login redirects to /tickets', async ({ page, mockApi }) => {
  await mockApi.get('/api/auth/me', { error: 'unauthorized' }, 401);
  await page.goto('/login');

  // Wait for login form to render (auth check completed).
  await expect(page.getByLabel('Username')).toBeVisible();

  // Mock login endpoint — no need to re-mock /auth/me since
  // AuthProvider.login() sets user from the POST response directly.
  await mockApi.post('/api/auth/login', TEST_USER);

  await page.getByLabel('Username').fill('testuser');
  await page.getByLabel('Password').fill('secret');
  await page.getByRole('button', { name: 'Sign in' }).click();

  await expect(page).toHaveURL(/\/tickets/);
});

test('failed login shows error message', async ({ page, mockApi }) => {
  await mockApi.get('/api/auth/me', { error: 'unauthorized' }, 401);
  await mockApi.post('/api/auth/login', { error: 'unauthorized' }, 401);

  await page.goto('/login');
  await page.getByLabel('Username').fill('wrong');
  await page.getByLabel('Password').fill('wrong');
  await page.getByRole('button', { name: 'Sign in' }).click();

  await expect(page.getByText('Invalid login or password.')).toBeVisible();
});

test('authenticated user reaches /tickets', async ({ page, mockApi }) => {
  await mockApi.loginAs();
  await page.goto('/');
  await expect(page).toHaveURL(/\/tickets/);
});
