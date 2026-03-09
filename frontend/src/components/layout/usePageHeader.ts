import { useContext, useEffect } from 'react';
import { PageHeaderContext, type PageHeaderConfig } from './pageHeaderState';

/** Called by page components to configure the header on mount. */
export function usePageHeader(config: PageHeaderConfig) {
  const ctx = useContext(PageHeaderContext);
  if (!ctx) throw new Error('usePageHeader must be used within PageHeaderProvider');
  const { setConfig } = ctx;
  useEffect(() => {
    setConfig(config);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setConfig, config.title, config.subtitle, ...config.breadcrumb]);
}

/** Called by the PageHeader component to read current values. */
export function usePageHeaderValue(): PageHeaderConfig | null {
  const ctx = useContext(PageHeaderContext);
  if (!ctx) throw new Error('usePageHeaderValue must be used within PageHeaderProvider');
  return ctx.config;
}
