import {
  useState,
  useCallback,
  useRef,
  useEffect,
  type ReactNode,
  type KeyboardEvent,
} from 'react';
import styles from './InlineText.module.css';

interface InlineTextProps {
  /** Current display value. */
  value: string;
  /** Called on save (Enter or blur). */
  onSave: (value: string) => void;
  /** Content shown in display mode. Defaults to value text. */
  children?: ReactNode;
  /** Placeholder when value is empty. */
  placeholder?: string;
  /** Accessible label for the input. */
  'aria-label'?: string;
}

/**
 * Click-to-edit text input.
 *
 * Click to enter edit mode. Enter or blur saves. Escape cancels.
 */
export default function InlineText({
  value,
  onSave,
  children,
  placeholder = 'None',
  'aria-label': ariaLabel,
}: InlineTextProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editing]);

  const startEditing = useCallback(() => {
    setDraft(value);
    setEditing(true);
  }, [value]);

  const save = useCallback(() => {
    setEditing(false);
    if (draft !== value) {
      onSave(draft);
    }
  }, [draft, value, onSave]);

  const cancel = useCallback(() => {
    setEditing(false);
  }, []);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        save();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        cancel();
      }
    },
    [save, cancel],
  );

  if (editing) {
    return (
      <div className={styles.wrapper}>
        <input
          ref={inputRef}
          type="text"
          className={styles.input}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={save}
          aria-label={ariaLabel}
        />
      </div>
    );
  }

  return (
    <div className={styles.wrapper}>
      <button
        type="button"
        className={styles.display}
        onClick={startEditing}
        aria-label={ariaLabel ? `Edit ${ariaLabel}` : undefined}
      >
        {children ?? (value ? value : <span className={styles.placeholder}>{placeholder}</span>)}
      </button>
    </div>
  );
}
