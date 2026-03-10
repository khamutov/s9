import { render, screen } from '@testing-library/react';
import type { Attachment } from '../api/types';
import AttachmentList from './AttachmentList';

const imageAttachment: Attachment = {
  id: 1,
  original_name: 'screenshot.png',
  mime_type: 'image/png',
  size_bytes: 245760,
  url: '/api/attachments/1/screenshot.png',
};

const fileAttachment: Attachment = {
  id: 2,
  original_name: 'debug.log',
  mime_type: 'text/plain',
  size_bytes: 8192,
  url: '/api/attachments/2/debug.log',
};

const pdfAttachment: Attachment = {
  id: 3,
  original_name: 'report.pdf',
  mime_type: 'application/pdf',
  size_bytes: 1536000,
  url: '/api/attachments/3/report.pdf',
};

describe('AttachmentList', () => {
  it('renders nothing when attachments is empty', () => {
    const { container } = render(<AttachmentList attachments={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders image thumbnails for image attachments', () => {
    render(<AttachmentList attachments={[imageAttachment]} />);
    expect(screen.getByText('Attachments')).toBeInTheDocument();
    const img = screen.getByAltText('screenshot.png');
    expect(img).toBeInTheDocument();
    expect(img.closest('a')).toHaveAttribute('href', '/api/attachments/1/screenshot.png');
  });

  it('renders file items for non-image attachments', () => {
    render(<AttachmentList attachments={[fileAttachment]} />);
    expect(screen.getByText('debug.log')).toBeInTheDocument();
    expect(screen.getByText('8.0 KB')).toBeInTheDocument();
  });

  it('renders both images and files together', () => {
    render(<AttachmentList attachments={[imageAttachment, fileAttachment, pdfAttachment]} />);
    expect(screen.getByAltText('screenshot.png')).toBeInTheDocument();
    expect(screen.getByText('debug.log')).toBeInTheDocument();
    expect(screen.getByText('report.pdf')).toBeInTheDocument();
    expect(screen.getByText('1.5 MB')).toBeInTheDocument();
  });

  it('treats SVG as file (not inline image preview)', () => {
    const svg: Attachment = {
      id: 4,
      original_name: 'diagram.svg',
      mime_type: 'image/svg+xml',
      size_bytes: 4096,
      url: '/api/attachments/4/diagram.svg',
    };
    render(<AttachmentList attachments={[svg]} />);
    // SVG should render as a file link, not an img
    expect(screen.queryByRole('img')).not.toBeInTheDocument();
    expect(screen.getByText('diagram.svg')).toBeInTheDocument();
  });

  it('file download links use force download param', () => {
    render(<AttachmentList attachments={[fileAttachment]} />);
    const link = screen.getByText('debug.log').closest('a');
    expect(link).toHaveAttribute('href', '/api/attachments/2/debug.log?download=1');
  });
});
