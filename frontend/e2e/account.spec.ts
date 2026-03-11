import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER } from './fixtures/mock-data';

test.describe('Account Page', () => {
  test('displays profile form with user data', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await page.goto('/account');
    await expect(page.getByRole('heading', { name: 'Account' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Profile' })).toBeVisible();
    await expect(page.getByText('testuser')).toBeVisible();
    await expect(page.locator('input[id="account-display-name"]')).toHaveValue('Test User');
    await expect(page.locator('input[id="account-email"]')).toHaveValue('test@s9.dev');
  });

  test('displays password change section', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await page.goto('/account');
    await expect(page.getByRole('heading', { name: 'Change Password' })).toBeVisible();
    await expect(page.getByRole('button', { name: /change password/i })).toBeVisible();
  });

  test('validates empty display name', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await page.goto('/account');
    await page.locator('input[id="account-display-name"]').clear();
    await page.getByRole('button', { name: /save profile/i }).click();
    await expect(page.getByText('Display name is required')).toBeVisible();
  });

  test('submits profile update', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.patch('/api/users/1', { ...TEST_USER, display_name: 'Updated Name' });
    // Re-mock auth/me to return updated user after refresh
    await page.goto('/account');
    await page.locator('input[id="account-display-name"]').fill('Updated Name');
    await page.getByRole('button', { name: /save profile/i }).click();
    await expect(page.getByText('Profile updated.')).toBeVisible();
  });

  test('validates password mismatch', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await page.goto('/account');
    await page.locator('input[id="account-current-password"]').fill('oldpass123');
    await page.locator('input[id="account-new-password"]').fill('newpass123');
    await page.locator('input[id="account-confirm-password"]').fill('different');
    await page.getByRole('button', { name: /change password/i }).click();
    await expect(page.getByText('Passwords do not match')).toBeVisible();
  });

  test('sidebar links to account page', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets', { items: [], has_more: false });
    await page.goto('/tickets');
    await page.getByText('Test User').click();
    await expect(page).toHaveURL(/\/account/);
  });
});
