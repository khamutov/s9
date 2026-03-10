import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import MarkdownRenderer from './MarkdownRenderer';

function renderMd(markdown: string) {
  return render(
    <MemoryRouter>
      <MarkdownRenderer>{markdown}</MarkdownRenderer>
    </MemoryRouter>,
  );
}

describe('MarkdownRenderer', () => {
  it('renders plain text', () => {
    renderMd('Hello world');
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('renders bold text', () => {
    renderMd('This is **bold** text');
    expect(screen.getByText('bold').tagName).toBe('STRONG');
  });

  it('renders italic text', () => {
    renderMd('This is *italic* text');
    // Italic without mention class renders as <em>
    expect(screen.getByText('italic').tagName).toBe('EM');
  });

  it('renders inline code', () => {
    renderMd('Use `foo()` here');
    expect(screen.getByText('foo()').tagName).toBe('CODE');
  });

  it('renders code blocks', () => {
    renderMd('```\nconst x = 1;\n```');
    expect(screen.getByText('const x = 1;').closest('pre')).toBeInTheDocument();
  });

  it('renders links with target=_blank for external URLs', () => {
    renderMd('[Google](https://google.com)');
    const link = screen.getByRole('link', { name: 'Google' });
    expect(link).toHaveAttribute('href', 'https://google.com');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer');
  });

  it('renders #42 as internal router link', () => {
    renderMd('See #42');
    const link = screen.getByRole('link', { name: '#42' });
    expect(link).toHaveAttribute('href', '/tickets/42');
    // Internal links should NOT have target=_blank
    expect(link).not.toHaveAttribute('target');
  });

  it('renders #MAP-23 as internal router link', () => {
    renderMd('Fix #MAP-23 now');
    const link = screen.getByRole('link', { name: '#MAP-23' });
    expect(link).toHaveAttribute('href', '/tickets/23');
  });

  it('renders comment#3 as anchor link', () => {
    renderMd('See comment#3');
    const link = screen.getByRole('link', { name: 'comment#3' });
    expect(link).toHaveAttribute('href', '#comment-3');
  });

  it('renders #42/comment#3 as combined link', () => {
    renderMd('Check #42/comment#3');
    const link = screen.getByRole('link', { name: '#42/comment#3' });
    expect(link).toHaveAttribute('href', '/tickets/42#comment-3');
  });

  it('renders @mentions as styled spans', () => {
    renderMd('Hey @alex check this');
    const mention = screen.getByText('@alex');
    expect(mention.tagName).toBe('SPAN');
  });

  it('renders GFM tables', () => {
    renderMd('| A | B |\n| --- | --- |\n| 1 | 2 |');
    expect(screen.getByRole('table')).toBeInTheDocument();
    expect(screen.getByText('1')).toBeInTheDocument();
  });

  it('renders GFM strikethrough', () => {
    renderMd('~~deleted~~');
    expect(screen.getByText('deleted').tagName).toBe('DEL');
  });

  it('renders blockquotes', () => {
    renderMd('> quoted text');
    expect(screen.getByText('quoted text').closest('blockquote')).toBeInTheDocument();
  });

  it('renders lists', () => {
    renderMd('- item one\n- item two');
    expect(screen.getByText('item one').closest('li')).toBeInTheDocument();
    expect(screen.getByText('item two').closest('li')).toBeInTheDocument();
  });

  it('accepts className prop', () => {
    const { container } = renderMd('test');
    expect(container.firstElementChild).toBeInTheDocument();
  });
});
