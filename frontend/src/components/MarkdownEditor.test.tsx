import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MarkdownEditor } from './MarkdownEditor';

vi.mock('../api/attachments', () => ({
  uploadAttachment: vi.fn(),
  attachmentUrl: vi.fn(
    (id: number, name: string) => `/api/attachments/${id}/${name}`,
  ),
}));

describe('MarkdownEditor', () => {
  let onChange: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onChange = vi.fn();
  });

  it('renders textarea with placeholder', () => {
    render(<MarkdownEditor value="" onChange={onChange} />);
    expect(
      screen.getByPlaceholderText(/Write a comment/),
    ).toBeInTheDocument();
  });

  it('renders toolbar buttons', () => {
    render(<MarkdownEditor value="" onChange={onChange} />);
    expect(screen.getByRole('button', { name: 'Bold' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Italic' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Code' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Link' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'List' })).toBeInTheDocument();
  });

  it('calls onChange when typing', async () => {
    render(<MarkdownEditor value="" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', { name: 'Markdown editor' });
    await userEvent.type(textarea, 'hello');
    expect(onChange).toHaveBeenCalled();
  });

  it('shows write/preview tabs', () => {
    render(<MarkdownEditor value="" onChange={onChange} />);
    expect(screen.getByRole('button', { name: 'Write' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Preview' })).toBeInTheDocument();
  });

  it('switches to preview mode and shows content', async () => {
    render(<MarkdownEditor value="Hello **world**" onChange={onChange} />);
    await userEvent.click(screen.getByRole('button', { name: 'Preview' }));
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(screen.getByText(/Hello \*\*world\*\*/)).toBeInTheDocument();
  });

  it('shows empty preview message when value is empty', async () => {
    render(<MarkdownEditor value="" onChange={onChange} />);
    await userEvent.click(screen.getByRole('button', { name: 'Preview' }));
    expect(screen.getByText('Nothing to preview')).toBeInTheDocument();
  });

  it('switches back to write mode', async () => {
    render(<MarkdownEditor value="text" onChange={onChange} />);
    await userEvent.click(screen.getByRole('button', { name: 'Preview' }));
    await userEvent.click(screen.getByRole('button', { name: 'Write' }));
    expect(screen.getByRole('textbox', { name: 'Markdown editor' })).toBeInTheDocument();
  });

  it('bold button wraps selection with **', () => {
    render(<MarkdownEditor value="hello world" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    // Simulate selecting "world"
    textarea.setSelectionRange(6, 11);
    fireEvent.click(screen.getByRole('button', { name: 'Bold' }));
    expect(onChange).toHaveBeenCalledWith('hello **world**');
  });

  it('italic button wraps selection with *', () => {
    render(<MarkdownEditor value="hello world" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(6, 11);
    fireEvent.click(screen.getByRole('button', { name: 'Italic' }));
    expect(onChange).toHaveBeenCalledWith('hello *world*');
  });

  it('link button wraps selection in link syntax', () => {
    render(<MarkdownEditor value="hello world" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(6, 11);
    fireEvent.click(screen.getByRole('button', { name: 'Link' }));
    expect(onChange).toHaveBeenCalledWith('hello [world](url)');
  });

  it('link button inserts template when nothing selected', () => {
    render(<MarkdownEditor value="hello " onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(6, 6);
    fireEvent.click(screen.getByRole('button', { name: 'Link' }));
    expect(onChange).toHaveBeenCalledWith('hello [text](url)');
  });

  it('list button inserts dash prefix', () => {
    render(<MarkdownEditor value="" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(0, 0);
    fireEvent.click(screen.getByRole('button', { name: 'List' }));
    expect(onChange).toHaveBeenCalledWith('- ');
  });

  it('code button wraps single-line selection in backticks', () => {
    render(<MarkdownEditor value="hello code" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(6, 10);
    fireEvent.click(screen.getByRole('button', { name: 'Code' }));
    expect(onChange).toHaveBeenCalledWith('hello `code`');
  });

  it('code button wraps multiline selection in code fence', () => {
    const multiline = 'line1\nline2';
    render(
      <MarkdownEditor value={multiline} onChange={onChange} />,
    );
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(0, multiline.length);
    fireEvent.click(screen.getByRole('button', { name: 'Code' }));
    expect(onChange).toHaveBeenCalledWith('```\nline1\nline2\n```');
  });

  it('Ctrl+B keyboard shortcut applies bold', () => {
    render(<MarkdownEditor value="text" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(0, 4);
    fireEvent.keyDown(textarea, { key: 'b', ctrlKey: true });
    expect(onChange).toHaveBeenCalledWith('**text**');
  });

  it('Ctrl+I keyboard shortcut applies italic', () => {
    render(<MarkdownEditor value="text" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(0, 4);
    fireEvent.keyDown(textarea, { key: 'i', ctrlKey: true });
    expect(onChange).toHaveBeenCalledWith('*text*');
  });

  it('Ctrl+K keyboard shortcut inserts link', () => {
    render(<MarkdownEditor value="text" onChange={onChange} />);
    const textarea = screen.getByRole('textbox', {
      name: 'Markdown editor',
    }) as HTMLTextAreaElement;

    textarea.setSelectionRange(0, 4);
    fireEvent.keyDown(textarea, { key: 'k', ctrlKey: true });
    expect(onChange).toHaveBeenCalledWith('[text](url)');
  });

  it('shows drag overlay when files are dragged over', () => {
    const { container } = render(<MarkdownEditor value="" onChange={onChange} />);
    const wrap = container.firstElementChild!;

    fireEvent.dragEnter(wrap, { dataTransfer: { files: [] } });
    expect(screen.getByTestId('drag-overlay')).toBeInTheDocument();
    expect(screen.getByText('Drop to upload')).toBeInTheDocument();

    fireEvent.dragLeave(wrap, { dataTransfer: { files: [] } });
    expect(screen.queryByTestId('drag-overlay')).not.toBeInTheDocument();
  });

  it('uploads file on drop and inserts image link', async () => {
    const { uploadAttachment } = await import('../api/attachments');
    const mockUpload = vi.mocked(uploadAttachment);
    mockUpload.mockResolvedValueOnce({
      id: 1,
      original_name: 'screenshot.png',
      mime_type: 'image/png',
      size_bytes: 1024,
      url: '/api/attachments/1/screenshot.png',
    });

    const { container } = render(<MarkdownEditor value="" onChange={onChange} />);
    const wrap = container.firstElementChild!;

    const file = new File(['bytes'], 'screenshot.png', { type: 'image/png' });
    fireEvent.drop(wrap, {
      dataTransfer: { files: [file], items: [], types: [] },
    });

    await vi.waitFor(() => {
      expect(mockUpload).toHaveBeenCalledWith(file);
    });

    await vi.waitFor(() => {
      expect(onChange).toHaveBeenCalledWith(
        '![screenshot.png](/api/attachments/1/screenshot.png)\n',
      );
    });
  });

  it('inserts non-image attachment as regular link', async () => {
    const { uploadAttachment } = await import('../api/attachments');
    const mockUpload = vi.mocked(uploadAttachment);
    mockUpload.mockResolvedValueOnce({
      id: 2,
      original_name: 'report.pdf',
      mime_type: 'application/pdf',
      size_bytes: 5000,
      url: '/api/attachments/2/report.pdf',
    });

    const { container } = render(<MarkdownEditor value="" onChange={onChange} />);
    const wrap = container.firstElementChild!;

    const file = new File(['bytes'], 'report.pdf', { type: 'application/pdf' });
    fireEvent.drop(wrap, {
      dataTransfer: { files: [file], items: [], types: [] },
    });

    await vi.waitFor(() => {
      expect(onChange).toHaveBeenCalledWith(
        '[report.pdf](/api/attachments/2/report.pdf)\n',
      );
    });
  });

  it('shows footer hint text', () => {
    render(<MarkdownEditor value="" onChange={onChange} />);
    expect(
      screen.getByText('Markdown supported · Drop files to attach'),
    ).toBeInTheDocument();
  });

  it('disables textarea and buttons when disabled', () => {
    render(<MarkdownEditor value="" onChange={onChange} disabled />);
    expect(screen.getByRole('textbox', { name: 'Markdown editor' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Bold' })).toBeDisabled();
  });

  it('custom placeholder text', () => {
    render(
      <MarkdownEditor value="" onChange={onChange} placeholder="Enter description" />,
    );
    expect(
      screen.getByPlaceholderText('Enter description'),
    ).toBeInTheDocument();
  });
});
