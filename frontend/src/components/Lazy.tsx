import { Suspense } from 'react';

/** Suspense wrapper for lazy-loaded route components. */
export default function Lazy({ children }: { children: React.ReactNode }) {
  return <Suspense fallback={null}>{children}</Suspense>;
}
