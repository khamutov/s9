import type { Attachment } from '../api/types';
import { attachmentUrl } from '../api/attachments';
import styles from './AttachmentList.module.css';

/** Props for the AttachmentList component. */
interface AttachmentListProps {
  attachments: Attachment[];
}

/** Format file size in human-readable form. */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Returns true if the MIME type is an image that can be previewed inline. */
function isImage(mimeType: string): boolean {
  return mimeType.startsWith('image/') && mimeType !== 'image/svg+xml';
}

/** Displays a list of file attachments with download links and image previews. */
export default function AttachmentList({ attachments }: AttachmentListProps) {
  if (attachments.length === 0) return null;

  const images = attachments.filter((a) => isImage(a.mime_type));
  const files = attachments.filter((a) => !isImage(a.mime_type));

  return (
    <div className={styles.wrap}>
      <div className={styles.label}>Attachments</div>

      {images.length > 0 && (
        <div className={styles.imageGrid}>
          {images.map((att) => (
            <a
              key={att.id}
              href={attachmentUrl(att.id, att.original_name)}
              target="_blank"
              rel="noopener noreferrer"
              className={styles.imageThumb}
              title={att.original_name}
            >
              <img src={attachmentUrl(att.id, att.original_name)} alt={att.original_name} />
            </a>
          ))}
        </div>
      )}

      {files.length > 0 && (
        <div className={styles.fileList}>
          {files.map((att) => (
            <a
              key={att.id}
              href={attachmentUrl(att.id, att.original_name, true)}
              className={styles.fileItem}
              title={`Download ${att.original_name}`}
            >
              <svg
                className={styles.fileIcon}
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M9 1.5H4a1 1 0 0 0-1 1v11a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V5.5L9 1.5z" />
                <path d="M9 1.5v4h4" />
              </svg>
              <span className={styles.fileName}>{att.original_name}</span>
              <span className={styles.fileSize}>{formatSize(att.size_bytes)}</span>
            </a>
          ))}
        </div>
      )}
    </div>
  );
}
