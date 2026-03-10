import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Link } from 'react-router';
import remarkS9References from './remarkS9References';
import styles from './MarkdownRenderer.module.css';
import type { ComponentProps } from 'react';

/** Props for the MarkdownRenderer component. */
export interface MarkdownRendererProps {
  /** Markdown source text to render. */
  children: string;
  /** Additional CSS class name. */
  className?: string;
}

/** Custom link component that uses React Router for internal links and renders mentions. */
function MdLink({
  href,
  children,
}: ComponentProps<'a'> & { href?: string; children?: React.ReactNode }) {
  if (href && href.startsWith('mention:')) {
    return <span className={styles.mention}>{children}</span>;
  }
  if (href && href.startsWith('/')) {
    return (
      <Link to={href} className={styles.reference}>
        {children}
      </Link>
    );
  }
  if (href && href.startsWith('#')) {
    return (
      <a href={href} className={styles.reference}>
        {children}
      </a>
    );
  }
  return (
    <a href={href} target="_blank" rel="noopener noreferrer">
      {children}
    </a>
  );
}

/** Allow mention: scheme URLs through react-markdown's URL sanitizer. */
function urlTransform(url: string): string {
  if (url.startsWith('mention:')) return url;
  // Default: allow http, https, mailto, tel, and relative URLs
  const safeProtocols = ['http:', 'https:', 'mailto:', 'tel:'];
  try {
    const parsed = new URL(url, 'http://localhost');
    if (safeProtocols.includes(parsed.protocol)) return url;
  } catch {
    // Relative URL — allow it
  }
  if (url.startsWith('/') || url.startsWith('#') || url.startsWith('.')) return url;
  return url;
}

const remarkPlugins = [remarkGfm, remarkS9References];
const components = {
  a: MdLink,
};

/**
 * Renders CommonMark markdown with GFM extensions and S9 micro-syntax links.
 * Converts #42, #MAP-23, comment#3, and @mentions into clickable elements.
 */
export default function MarkdownRenderer({ children, className }: MarkdownRendererProps) {
  return (
    <div className={`${styles.prose} ${className ?? ''}`}>
      <ReactMarkdown
        remarkPlugins={remarkPlugins}
        components={components}
        urlTransform={urlTransform}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
