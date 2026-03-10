import {
  useCallback,
  useRef,
  useState,
  type DragEvent,
  type KeyboardEvent,
  type ClipboardEvent,
} from 'react';
import { uploadAttachment } from '../api/attachments';
import { attachmentUrl } from '../api/attachments';
import styles from './MarkdownEditor.module.css';

/** Props for the MarkdownEditor component. */
export interface MarkdownEditorProps {
  /** Current markdown text value. */
  value: string;
  /** Called when the text content changes. */
  onChange: (value: string) => void;
  /** Textarea placeholder text. */
  placeholder?: string;
  /** Minimum height for the textarea in pixels. */
  minHeight?: number;
  /** Whether the editor is disabled. */
  disabled?: boolean;
}

/**
 * Markdown editor with toolbar, write/preview tabs, keyboard shortcuts,
 * and attachment upload via drag-drop and paste.
 */
export function MarkdownEditor({
  value,
  onChange,
  placeholder = 'Write a comment… Use @mentions and #references',
  minHeight,
  disabled = false,
}: MarkdownEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [mode, setMode] = useState<'write' | 'preview'>('write');
  const [isDragging, setIsDragging] = useState(false);
  const [uploadCount, setUploadCount] = useState(0);
  const dragCounter = useRef(0);

  /** Insert text at the current cursor position, replacing selection if any. */
  const insertAtCursor = useCallback(
    (before: string, after: string = '') => {
      const ta = textareaRef.current;
      if (!ta) return;

      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      const selected = value.slice(start, end);
      const insertion = before + selected + after;
      const newValue = value.slice(0, start) + insertion + value.slice(end);
      onChange(newValue);

      // Restore cursor position after React re-render
      requestAnimationFrame(() => {
        ta.focus();
        if (selected) {
          ta.setSelectionRange(start + before.length, start + before.length + selected.length);
        } else {
          const cursor = start + before.length;
          ta.setSelectionRange(cursor, cursor);
        }
      });
    },
    [value, onChange],
  );

  /** Wrap selection with markdown formatting markers. */
  const wrapSelection = useCallback(
    (marker: string) => {
      insertAtCursor(marker, marker);
    },
    [insertAtCursor],
  );

  /** Insert a link template. */
  const insertLink = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;

    const selected = value.slice(ta.selectionStart, ta.selectionEnd);
    if (selected) {
      insertAtCursor('[', '](url)');
    } else {
      insertAtCursor('[text](url)');
    }
  }, [value, insertAtCursor]);

  /** Insert a list marker at line start. */
  const insertList = useCallback(() => {
    insertAtCursor('- ');
  }, [insertAtCursor]);

  /** Insert a code block or inline code. */
  const insertCode = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;

    const selected = value.slice(ta.selectionStart, ta.selectionEnd);
    if (selected.includes('\n')) {
      insertAtCursor('```\n', '\n```');
    } else {
      wrapSelection('`');
    }
  }, [value, insertAtCursor, wrapSelection]);

  /** Upload a file and insert markdown link at cursor. */
  const handleFileUpload = useCallback(
    async (file: File) => {
      setUploadCount((c) => c + 1);
      try {
        const attachment = await uploadAttachment(file);
        const url = attachmentUrl(attachment.id, attachment.original_name);
        const isImage = attachment.mime_type.startsWith('image/');
        const mdLink = isImage
          ? `![${attachment.original_name}](${url})`
          : `[${attachment.original_name}](${url})`;

        const ta = textareaRef.current;
        const pos = ta ? ta.selectionStart : value.length;
        const needsNewline = pos > 0 && value[pos - 1] !== '\n' ? '\n' : '';
        const newValue = value.slice(0, pos) + needsNewline + mdLink + '\n' + value.slice(pos);
        onChange(newValue);
      } catch {
        // Upload failed — silently ignore (user sees no insertion)
      } finally {
        setUploadCount((c) => c - 1);
      }
    },
    [value, onChange],
  );

  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault();
    dragCounter.current++;
    if (dragCounter.current === 1) setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    dragCounter.current--;
    if (dragCounter.current === 0) setIsDragging(false);
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      dragCounter.current = 0;
      setIsDragging(false);

      const files = Array.from(e.dataTransfer.files);
      for (const file of files) {
        handleFileUpload(file);
      }
    },
    [handleFileUpload],
  );

  const handlePaste = useCallback(
    (e: ClipboardEvent) => {
      const files = Array.from(e.clipboardData.files);
      if (files.length > 0) {
        e.preventDefault();
        for (const file of files) {
          handleFileUpload(file);
        }
      }
    },
    [handleFileUpload],
  );

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;

      switch (e.key) {
        case 'b':
          e.preventDefault();
          wrapSelection('**');
          break;
        case 'i':
          e.preventDefault();
          wrapSelection('*');
          break;
        case 'k':
          e.preventDefault();
          insertLink();
          break;
      }
    },
    [wrapSelection, insertLink],
  );

  return (
    <div
      className={`${styles.wrap} ${styles.wrapRelative}`}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {/* Drag overlay */}
      {isDragging && (
        <div className={styles.dragOverlay} data-testid="drag-overlay">
          <span className={styles.dragLabel}>Drop to upload</span>
        </div>
      )}

      {/* Toolbar */}
      <div className={styles.toolbar}>
        <button
          type="button"
          className={styles.toolbarBtn}
          title="Bold (Ctrl+B)"
          onClick={() => wrapSelection('**')}
          disabled={disabled}
          aria-label="Bold"
        >
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M4 2.5h5a3 3 0 0 1 0 6H4zM4 8.5h6a3 3 0 0 1 0 6H4z" />
          </svg>
        </button>
        <button
          type="button"
          className={styles.toolbarBtn}
          title="Italic (Ctrl+I)"
          onClick={() => wrapSelection('*')}
          disabled={disabled}
          aria-label="Italic"
        >
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M10 2.5H6M10 13.5H6M9.5 2.5L6.5 13.5" />
          </svg>
        </button>
        <button
          type="button"
          className={styles.toolbarBtn}
          title="Code"
          onClick={insertCode}
          disabled={disabled}
          aria-label="Code"
        >
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M5 4L1.5 8L5 12M11 4l3.5 4L11 12" />
          </svg>
        </button>
        <button
          type="button"
          className={styles.toolbarBtn}
          title="Link (Ctrl+K)"
          onClick={insertLink}
          disabled={disabled}
          aria-label="Link"
        >
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M6.5 9.5a3.5 3.5 0 0 0 5 0l2-2a3.5 3.5 0 0 0-5-5l-1 1M9.5 6.5a3.5 3.5 0 0 0-5 0l-2 2a3.5 3.5 0 0 0 5 5l1-1" />
          </svg>
        </button>
        <button
          type="button"
          className={styles.toolbarBtn}
          title="List"
          onClick={insertList}
          disabled={disabled}
          aria-label="List"
        >
          <svg
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
          >
            <path d="M5.5 4h8M5.5 8h8M5.5 12h8" />
            <circle cx="2.5" cy="4" r="0.75" fill="currentColor" stroke="none" />
            <circle cx="2.5" cy="8" r="0.75" fill="currentColor" stroke="none" />
            <circle cx="2.5" cy="12" r="0.75" fill="currentColor" stroke="none" />
          </svg>
        </button>

        <div className={styles.toolbarSep} />

        {/* Write / Preview tabs */}
        <div className={styles.tabBar}>
          <button
            type="button"
            className={`${styles.tab} ${mode === 'write' ? styles.tabActive : ''}`}
            onClick={() => setMode('write')}
          >
            Write
          </button>
          <button
            type="button"
            className={`${styles.tab} ${mode === 'preview' ? styles.tabActive : ''}`}
            onClick={() => setMode('preview')}
          >
            Preview
          </button>
        </div>
      </div>

      {/* Editor body */}
      {mode === 'write' ? (
        <textarea
          ref={textareaRef}
          className={styles.textarea}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          placeholder={placeholder}
          disabled={disabled}
          style={minHeight ? { minHeight } : undefined}
          aria-label="Markdown editor"
        />
      ) : (
        <div className={styles.preview}>
          {value ? (
            <pre style={{ whiteSpace: 'pre-wrap', fontFamily: 'inherit' }}>{value}</pre>
          ) : (
            <span className={styles.previewEmpty}>Nothing to preview</span>
          )}
        </div>
      )}

      {/* Footer */}
      <div className={styles.footer}>
        <span className={styles.hint}>
          {uploadCount > 0 ? (
            <span className={styles.uploading}>Uploading…</span>
          ) : (
            'Markdown supported · Drop files to attach'
          )}
        </span>
      </div>
    </div>
  );
}
