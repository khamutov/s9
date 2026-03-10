import { render, screen, fireEvent } from '@testing-library/react';
import { vi } from 'vitest';
import InlineSelect, { type SelectOption } from './InlineSelect';

const options: SelectOption<string>[] = [
  { value: 'a', label: 'Alpha' },
  { value: 'b', label: 'Beta' },
  { value: 'c', label: 'Gamma' },
];

describe('InlineSelect', () => {
  it('renders current value in display mode', () => {
    render(<InlineSelect value="a" options={options} onChange={() => {}} />);
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('opens dropdown on click', () => {
    render(<InlineSelect value="a" options={options} onChange={() => {}} />);
    fireEvent.click(screen.getByText('Alpha'));
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    // All options visible
    expect(screen.getAllByRole('option')).toHaveLength(3);
  });

  it('calls onChange with new value on selection', () => {
    const onChange = vi.fn();
    render(<InlineSelect value="a" options={options} onChange={onChange} />);
    fireEvent.click(screen.getByText('Alpha'));
    fireEvent.click(screen.getByRole('option', { name: 'Beta' }));
    expect(onChange).toHaveBeenCalledWith('b');
  });

  it('does not call onChange when selecting current value', () => {
    const onChange = vi.fn();
    render(<InlineSelect value="a" options={options} onChange={onChange} />);
    fireEvent.click(screen.getByText('Alpha'));
    fireEvent.click(screen.getByRole('option', { name: 'Alpha' }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it('closes dropdown on Escape', () => {
    render(<InlineSelect value="a" options={options} onChange={() => {}} />);
    fireEvent.click(screen.getByText('Alpha'));
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole('listbox').parentElement!, {
      key: 'Escape',
    });
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('closes dropdown when clicking backdrop', () => {
    render(<InlineSelect value="a" options={options} onChange={() => {}} />);
    fireEvent.click(screen.getByText('Alpha'));
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    // Backdrop is the fixed-position overlay
    const backdrop = document.querySelector('[class*="backdrop"]')!;
    fireEvent.click(backdrop);
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('marks current value as aria-selected', () => {
    render(<InlineSelect value="b" options={options} onChange={() => {}} />);
    fireEvent.click(screen.getByText('Beta'));
    const selected = screen.getByRole('option', { name: 'Beta' });
    expect(selected).toHaveAttribute('aria-selected', 'true');
    const unselected = screen.getByRole('option', { name: 'Alpha' });
    expect(unselected).toHaveAttribute('aria-selected', 'false');
  });

  it('uses custom renderValue and renderOption', () => {
    render(
      <InlineSelect
        value="a"
        options={options}
        onChange={() => {}}
        renderValue={(v) => <span data-testid="custom-value">{v}</span>}
        renderOption={(v, l) => <span data-testid="custom-opt">{l}!</span>}
      />,
    );
    expect(screen.getByTestId('custom-value')).toHaveTextContent('a');
    fireEvent.click(screen.getByTestId('custom-value'));
    expect(screen.getAllByTestId('custom-opt')).toHaveLength(3);
  });
});
