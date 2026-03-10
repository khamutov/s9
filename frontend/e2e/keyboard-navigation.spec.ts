import { test, expect } from './fixtures/test-fixtures';
import type { Ticket, CursorPage } from '../src/api/types';

const makeTicket = (overrides: Partial<Ticket> = {}): Ticket => ({
  id: 1,
  title: 'Test ticket',
  slug: 'S9-1',
  type: 'bug',
  status: 'new',
  priority: 'P2',
  owner: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  component: { id: 1, name: 'Auth', path: 'Auth', slug: 'AUTH' },
  created_by: { id: 1, login: 'alex', display_name: 'Alex Kim' },
  cc: [],
  milestones: [],
  comment_count: 0,
  estimation_hours: null,
  estimation_display: null,
  created_at: '2026-03-10T10:00:00Z',
  updated_at: '2026-03-10T10:00:00Z',
  ...overrides,
});

const MOCK_TICKETS: CursorPage<Ticket> = {
  items: [
    makeTicket({ id: 1, title: 'First ticket', slug: 'S9-1' }),
    makeTicket({ id: 2, title: 'Second ticket', slug: 'S9-2' }),
    makeTicket({ id: 3, title: 'Third ticket', slug: 'S9-3' }),
  ],
  has_more: false,
};

test.describe('Keyboard Navigation', () => {
  test('j/k keys navigate through ticket rows', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', MOCK_TICKETS);
    await page.goto('/tickets');

    await expect(page.getByText('First ticket')).toBeVisible();

    // Click body to ensure no input is focused
    await page.locator('body').click();

    // Press j to select first row — verify via background color change
    await page.keyboard.press('j');
    const firstRow = page.locator('tbody tr').nth(0);
    await expect(firstRow).toHaveCSS('background-color', 'rgb(36, 35, 32)'); // --bg-hover

    // Press j again to select second row
    await page.keyboard.press('j');
    const secondRow = page.locator('tbody tr').nth(1);
    await expect(secondRow).toHaveCSS('background-color', 'rgb(36, 35, 32)');

    // Press k to go back to first row
    await page.keyboard.press('k');
    await expect(firstRow).toHaveCSS('background-color', 'rgb(36, 35, 32)');
  });

  test('Enter navigates to selected ticket', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', MOCK_TICKETS);
    await page.goto('/tickets');

    await expect(page.getByText('First ticket')).toBeVisible();

    // Click body to ensure no input is focused
    await page.locator('body').click();

    // Select first row and press Enter
    await page.keyboard.press('j');
    await page.keyboard.press('Enter');

    await expect(page).toHaveURL(/\/tickets\/1/);
  });

  test('c key navigates to create ticket page', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', { items: [], has_more: false });
    await page.goto('/tickets');

    await expect(page.getByText('No tickets found.')).toBeVisible();

    // Click body to ensure no input is focused
    await page.locator('body').click();

    await page.keyboard.press('c');
    await expect(page).toHaveURL(/\/tickets\/new/);
  });

  test('keyboard shortcuts are disabled when filter input is focused', async ({
    page,
    mockApi,
  }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', MOCK_TICKETS);
    await page.goto('/tickets');

    await expect(page.getByText('First ticket')).toBeVisible();

    // Focus filter input
    const filterInput = page.getByRole('textbox', { name: /filter tickets/i });
    await filterInput.click();
    await expect(filterInput).toBeFocused();

    // j key should type in input, not navigate rows
    await page.keyboard.press('j');
    await expect(filterInput).toHaveValue('j');
  });

  test('/ key focuses filter bar from any page context', async ({ page, mockApi }) => {
    await mockApi.loginAs();
    await mockApi.get('/api/tickets', MOCK_TICKETS);
    await page.goto('/tickets');

    await expect(page.getByText('First ticket')).toBeVisible();

    const filterInput = page.getByRole('textbox', { name: /filter tickets/i });
    await expect(filterInput).not.toBeFocused();

    await page.keyboard.press('/');
    await expect(filterInput).toBeFocused();
  });
});
