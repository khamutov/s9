/** Error returned by the S9 API (matches DD 0.4 §5.3 error envelope). */
export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    public details?: Record<string, string>,
  ) {
    super(`API error ${status}: ${code}`);
    this.name = 'ApiError';
  }
}

/** Typed fetch wrapper for JSON API requests. */
export async function apiRequest<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: 'same-origin',
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new ApiError(res.status, err.error ?? 'unknown', err.details);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}
