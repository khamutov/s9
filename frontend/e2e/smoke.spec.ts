import { test, expect } from '@playwright/test';

test('app loads and redirects to /tickets', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveURL(/\/tickets/);
  await expect(page.locator('aside').getByText('S9')).toBeVisible();
  await expect(page.getByText('Tickets', { exact: true })).toBeVisible();
});

test('sidebar navigation works', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Components' }).click();
  await expect(page).toHaveURL(/\/components/);
  await expect(page.getByText('Components')).toBeVisible();
});
