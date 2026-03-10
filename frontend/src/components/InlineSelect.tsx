import {
  useState,
  useCallback,
  useEffect,
  useRef,
  type ReactNode,
  type KeyboardEvent,
} from 'react';
import styles from './InlineSelect.module.css';

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

interface InlineSelectProps<T extends string> {
  /** Current value. */
  value: T;
  /** Available options. */
  options: SelectOption<T>[];
  /** Called when the user picks a new value. */
  onChange: (value: T) => void;
  /** Render the display-mode content. Defaults to option label. */
  renderValue?: (value: T) => ReactNode;
  /** Render each dropdown option. Defaults to option label. */
  renderOption?: (value: T, label: string) => ReactNode;
  /** Accessible label for the trigger. */
  'aria-label'?: string;
}

/**
 * Click-to-edit select field.
 *
 * Display mode shows current value. Click opens a dropdown.
 * Selecting an option fires onChange and closes. Escape cancels.
 */
export default function InlineSelect<T extends string>({
  value,
  options,
  onChange,
  renderValue,
  renderOption,
  'aria-label': ariaLabel,
}: InlineSelectProps<T>) {
  const [open, setOpen] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const [focusIdx, setFocusIdx] = useState(-1);

  const close = useCallback(() => {
    setOpen(false);
    setFocusIdx(-1);
  }, []);

  const handleSelect = useCallback(
    (v: T) => {
      if (v !== value) {
        onChange(v);
      }
      close();
    },
    [value, onChange, close],
  );

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!open) return;
      if (e.key === 'Escape') {
        e.stopPropagation();
        close();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        setFocusIdx((i) => Math.min(i + 1, options.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setFocusIdx((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && focusIdx >= 0) {
        e.preventDefault();
        handleSelect(options[focusIdx].value);
      }
    },
    [open, close, focusIdx, options, handleSelect],
  );

  // Focus the active option when keyboard-navigating
  useEffect(() => {
    if (open && focusIdx >= 0 && listRef.current) {
      const el = listRef.current.children[focusIdx] as HTMLElement;
      el?.focus();
    }
  }, [open, focusIdx]);

  const currentOption = options.find((o) => o.value === value);

  return (
    <div className={styles.wrapper} onKeyDown={handleKeyDown} role="group">
      <button
        type="button"
        className={styles.display}
        onClick={() => setOpen(true)}
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        {renderValue ? renderValue(value) : (currentOption?.label ?? value)}
      </button>

      {open && (
        <>
          <div className={styles.backdrop} onClick={close} role="presentation" />
          <div className={styles.dropdown} role="listbox" ref={listRef}>
            {options.map((opt) => (
              <button
                key={opt.value}
                type="button"
                role="option"
                className={styles.option}
                aria-selected={opt.value === value}
                onClick={() => handleSelect(opt.value)}
              >
                {renderOption ? renderOption(opt.value, opt.label) : opt.label}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
