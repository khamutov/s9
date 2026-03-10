import { ApiError } from './client';
import type { Attachment } from './types';

/** Upload a file attachment. Returns metadata with a URL for linking to comments. */
export async function uploadAttachment(file: File): Promise<Attachment> {
  const form = new FormData();
  form.append('file', file);

  const res = await fetch('/api/attachments', {
    method: 'POST',
    credentials: 'same-origin',
    body: form,
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new ApiError(res.status, err.error ?? 'unknown', err.details);
  }

  return res.json();
}

/** Build the download URL for an attachment. */
export function attachmentUrl(id: number, filename: string, forceDownload = false): string {
  const base = `/api/attachments/${id}/${encodeURIComponent(filename)}`;
  return forceDownload ? `${base}?download=1` : base;
}
