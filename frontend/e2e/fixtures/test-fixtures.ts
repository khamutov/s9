import { test as base, expect } from '@playwright/test';
import type { Page } from '@playwright/test';
import type { AuthUser } from '../../src/api/types';
import { TEST_USER } from './mock-data';

interface MockApi {
  /** Mock a GET endpoint. */
  get(path: string, body: unknown, status?: number): Promise<void>;
  /** Mock a POST endpoint. */
  post(path: string, body: unknown, status?: number): Promise<void>;
  /** Mock an endpoint for a specific HTTP method. */
  route(method: string, path: string, body: unknown, status?: number): Promise<void>;
  /** Shortcut: mock GET /api/auth/me to return the given user. */
  loginAs(user?: AuthUser): Promise<void>;
}

function createMockApi(page: Page): MockApi {
  const api: MockApi = {
    async get(path, body, status = 200) {
      await api.route('GET', path, body, status);
    },

    async post(path, body, status = 200) {
      await api.route('POST', path, body, status);
    },

    async route(method, path, body, status = 200) {
      const upperMethod = method.toUpperCase();
      const normalizedPath = path.replace(/^\/?api\//, '');
      await page.route(`**/api/${normalizedPath}`, async (route) => {
        if (route.request().method() === upperMethod) {
          await route.fulfill({
            status,
            contentType: 'application/json',
            body: JSON.stringify(body),
          });
        } else {
          await route.fallback();
        }
      });
    },

    async loginAs(user = TEST_USER) {
      await api.get('/api/auth/me', user);
    },
  };

  return api;
}

/** Extended Playwright test with `mockApi` fixture for intercepting API calls. */
export const test = base.extend<{ mockApi: MockApi }>({
  mockApi: async ({ page }, use) => {
    await use(createMockApi(page));
  },
});

export { expect };
