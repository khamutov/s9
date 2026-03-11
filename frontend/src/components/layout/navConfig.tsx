import type { ReactNode } from 'react';

export interface NavItem {
  label: string;
  path: string;
  icon: ReactNode;
  /** If true, match path exactly (index routes). */
  end?: boolean;
}

export interface NavSection {
  label: string;
  items: NavItem[];
}

/* SVG icons extracted from prototypes/05-layout.html */
const iconProps = {
  className: 'nav-icon',
  viewBox: '0 0 16 16',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.5,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
};

export const NAV_SECTIONS: NavSection[] = [
  {
    label: 'Workspace',
    items: [
      {
        label: 'Tickets',
        path: '/tickets',
        end: true,
        icon: (
          <svg {...iconProps}>
            <path d="M2.5 4h11M2.5 8h11M2.5 12h7" />
          </svg>
        ),
      },
      {
        label: 'Components',
        path: '/components',
        icon: (
          <svg {...iconProps}>
            <rect x="1.5" y="1.5" width="5" height="5" rx="1" />
            <rect x="9.5" y="1.5" width="5" height="5" rx="1" />
            <rect x="1.5" y="9.5" width="5" height="5" rx="1" />
            <rect x="9.5" y="9.5" width="5" height="5" rx="1" />
          </svg>
        ),
      },
      {
        label: 'Milestones',
        path: '/milestones',
        icon: (
          <svg {...iconProps}>
            <path d="M2.5 14V2l9 4-9 4" />
          </svg>
        ),
      },
    ],
  },
  {
    label: 'Views',
    items: [
      {
        label: 'My Tickets',
        path: '/tickets?owner=me',
        icon: (
          <svg {...iconProps}>
            <circle cx="8" cy="5" r="3" />
            <path d="M2 14c0-3 2.5-5 6-5s6 2 6 5" />
          </svg>
        ),
      },
    ],
  },
  {
    label: 'Settings',
    items: [
      {
        label: 'Admin',
        path: '/admin',
        icon: (
          <svg {...iconProps}>
            <circle cx="8" cy="8" r="2.5" />
            <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
          </svg>
        ),
      },
    ],
  },
];
