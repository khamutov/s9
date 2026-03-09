import { createContext } from 'react';

export interface PageHeaderConfig {
  /** Breadcrumb segments — "S9" is always prepended by PageHeader. */
  breadcrumb: string[];
  title: string;
  subtitle?: string;
}

export const PageHeaderContext = createContext<{
  config: PageHeaderConfig | null;
  setConfig: (c: PageHeaderConfig) => void;
} | null>(null);
