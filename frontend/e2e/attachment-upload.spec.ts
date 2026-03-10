import { test, expect } from './fixtures/test-fixtures';
import { TEST_USER } from './fixtures/mock-data';

const mockTicket = {
  id: 42,
  title: 'Test Ticket',
  slug: 'S9-42',
  type: 'bug',
  status: 'new',
  priority: 'P2',
  owner: { id: 1, login: 'testuser', display_name: 'Test User' },
  component: { id: 1, name: 'Core', path: '/Core/' },
  created_by: { id: 1, login: 'testuser', display_name: 'Test User' },
  cc: [],
  milestones: [],
  estimation_hours: null,
  estimation_display: null,
  comment_count: 1,
  created_at: '2026-03-04T10:00:00.000Z',
  updated_at: '2026-03-04T10:00:00.000Z',
};

const mockComments = {
  items: [
    {
      id: 1,
      ticket_id: 42,
      number: 0,
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      body: 'Description text',
      attachments: [],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-04T10:00:00.000Z',
      updated_at: '2026-03-04T10:00:00.000Z',
    },
  ],
};

const mockCommentsWithAttachments = {
  items: [
    {
      id: 1,
      ticket_id: 42,
      number: 0,
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      body: 'Description with screenshot',
      attachments: [
        {
          id: 5,
          original_name: 'screenshot.png',
          mime_type: 'image/png',
          size_bytes: 245760,
          url: '/api/attachments/5/screenshot.png',
        },
      ],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-04T10:00:00.000Z',
      updated_at: '2026-03-04T10:00:00.000Z',
    },
    {
      id: 2,
      ticket_id: 42,
      number: 1,
      author: { id: 1, login: 'testuser', display_name: 'Test User' },
      body: 'See the attached log.',
      attachments: [
        {
          id: 10,
          original_name: 'app.log',
          mime_type: 'text/plain',
          size_bytes: 8192,
          url: '/api/attachments/10/app.log',
        },
      ],
      edit_count: 0,
      edits: [],
      created_at: '2026-03-04T11:00:00.000Z',
      updated_at: '2026-03-04T11:00:00.000Z',
    },
  ],
};

test.describe('Attachment display on ticket detail', () => {
  test('displays image attachment as thumbnail on description', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', mockTicket);
    await mockApi.get('/api/tickets/42/comments', mockCommentsWithAttachments);

    await page.goto('/tickets/42');

    // Image thumbnail should be visible
    const img = page.getByAltText('screenshot.png');
    await expect(img).toBeVisible();
  });

  test('displays file attachment with name and size on comment', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', mockTicket);
    await mockApi.get('/api/tickets/42/comments', mockCommentsWithAttachments);

    await page.goto('/tickets/42');

    // File attachment on comment #1
    await expect(page.getByText('app.log')).toBeVisible();
    await expect(page.getByText('8.0 KB')).toBeVisible();
  });

  test('shows "Attachments" label when comment has attachments', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', mockTicket);
    await mockApi.get('/api/tickets/42/comments', mockCommentsWithAttachments);

    await page.goto('/tickets/42');

    // "Attachments" label appears for both description and comment
    const labels = page.getByText('Attachments');
    await expect(labels.first()).toBeVisible();
  });

  test('does not show attachments section when comment has none', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', mockTicket);
    await mockApi.get('/api/tickets/42/comments', mockComments);

    await page.goto('/tickets/42');

    await expect(page.getByText('Description text')).toBeVisible();
    // No "Attachments" label should appear
    await expect(page.getByText('Attachments')).not.toBeVisible();
  });

  test('markdown editor shows drop hint', async ({ page, mockApi }) => {
    await mockApi.loginAs(TEST_USER);
    await mockApi.get('/api/tickets/42', mockTicket);
    await mockApi.get('/api/tickets/42/comments', mockComments);

    await page.goto('/tickets/42');

    // The editor footer hint
    await expect(page.getByText('Markdown supported · Drop files to attach')).toBeVisible();
  });
});
