import { render, screen, fireEvent } from '@testing-library/react';
import { vi } from 'vitest';
import InlineText from './InlineText';

describe('InlineText', () => {
  it('renders value in display mode', () => {
    render(<InlineText value="2d" onSave={() => {}} />);
    expect(screen.getByText('2d')).toBeInTheDocument();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('shows placeholder when value is empty', () => {
    render(<InlineText value="" onSave={() => {}} placeholder="None" />);
    expect(screen.getByText('None')).toBeInTheDocument();
  });

  it('enters edit mode on click', () => {
    render(<InlineText value="2d" onSave={() => {}} />);
    fireEvent.click(screen.getByText('2d'));
    const input = screen.getByRole('textbox');
    expect(input).toBeInTheDocument();
    expect(input).toHaveValue('2d');
  });

  it('saves on Enter', () => {
    const onSave = vi.fn();
    render(<InlineText value="2d" onSave={onSave} />);
    fireEvent.click(screen.getByText('2d'));
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: '4d' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onSave).toHaveBeenCalledWith('4d');
    // Returns to display mode
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('cancels on Escape without saving', () => {
    const onSave = vi.fn();
    render(<InlineText value="2d" onSave={onSave} />);
    fireEvent.click(screen.getByText('2d'));
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: '4d' } });
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('saves on blur', () => {
    const onSave = vi.fn();
    render(<InlineText value="2d" onSave={onSave} />);
    fireEvent.click(screen.getByText('2d'));
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: '1w' } });
    fireEvent.blur(input);
    expect(onSave).toHaveBeenCalledWith('1w');
  });

  it('does not call onSave if value unchanged', () => {
    const onSave = vi.fn();
    render(<InlineText value="2d" onSave={onSave} />);
    fireEvent.click(screen.getByText('2d'));
    fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Enter' });
    expect(onSave).not.toHaveBeenCalled();
  });

  it('renders custom children in display mode', () => {
    render(
      <InlineText value="2d" onSave={() => {}}>
        <span data-testid="custom">2 days</span>
      </InlineText>,
    );
    expect(screen.getByTestId('custom')).toHaveTextContent('2 days');
  });
});
