import { useState, type ReactNode } from 'react';
import { PageHeaderContext, type PageHeaderConfig } from './pageHeaderState';

/** Wraps the layout tree so child pages can set the page header. */
export function PageHeaderProvider({ children }: { children: ReactNode }) {
  const [config, setConfig] = useState<PageHeaderConfig | null>(null);
  return (
    <PageHeaderContext.Provider value={{ config, setConfig }}>
      {children}
    </PageHeaderContext.Provider>
  );
}
