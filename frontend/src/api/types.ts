/** Shared API types matching the backend response shapes. */

// --- Compact / embedded types ---

/** Compact user reference embedded in tickets, comments, etc. */
export interface CompactUser {
  id: number;
  login: string;
  display_name: string;
}

/** Compact component reference embedded in ticket responses. */
export interface CompactComponent {
  id: number;
  name: string;
  path: string;
  slug?: string | null;
  effective_slug?: string | null;
}

/** Compact milestone reference embedded in ticket responses. */
export interface CompactMilestone {
  id: number;
  name: string;
}

// --- Pagination ---

/** Cursor-based page (structured queries without free-text search). */
export interface CursorPage<T> {
  items: T[];
  next_cursor?: string;
  has_more: boolean;
}

/** Offset-based page (free-text search with BM25 ranking). */
export interface OffsetPage<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
}

/** Discriminated union returned by the ticket list endpoint. */
export type SearchResult<T> = CursorPage<T> | OffsetPage<T>;

/** Type guard: returns true when the result uses offset pagination. */
export function isOffsetPage<T>(r: SearchResult<T>): r is OffsetPage<T> {
  return 'total' in r;
}

// --- Enums ---

export type TicketType = 'bug' | 'feature' | 'task' | 'improvement';
export type TicketStatus = 'new' | 'in_progress' | 'verify' | 'done';
export type Priority = 'P0' | 'P1' | 'P2' | 'P3' | 'P4' | 'P5';
export type UserRole = 'admin' | 'user';
export type MilestoneStatus = 'open' | 'closed';

// --- Tickets ---

export interface Ticket {
  id: number;
  slug?: string | null;
  type: TicketType;
  title: string;
  status: TicketStatus;
  priority: Priority;
  owner: CompactUser;
  component: CompactComponent;
  estimation_hours?: number | null;
  estimation_display?: string | null;
  created_by: CompactUser;
  cc: CompactUser[];
  milestones: CompactMilestone[];
  comment_count: number;
  created_at: string;
  updated_at: string;
}

export interface CreateTicketRequest {
  type: TicketType;
  title: string;
  owner_id: number;
  component_id: number;
  priority?: Priority;
  description?: string;
  cc?: number[];
  milestones?: number[];
  estimation?: string;
}

export interface UpdateTicketRequest {
  title?: string;
  status?: TicketStatus;
  priority?: Priority;
  owner_id?: number;
  component_id?: number;
  type?: TicketType;
  cc?: number[];
  milestones?: number[];
  estimation?: string | null;
}

// --- Comments ---

export interface CommentEdit {
  old_body: string;
  edited_at: string;
}

export interface Comment {
  id: number;
  ticket_id: number;
  number: number;
  author: CompactUser;
  body: string;
  attachments: Attachment[];
  edit_count: number;
  edits: CommentEdit[];
  created_at: string;
  updated_at: string;
}

export interface CreateCommentRequest {
  body: string;
  attachment_ids?: number[];
}

export interface EditCommentRequest {
  body: string;
}

// --- Components ---

export interface Component {
  id: number;
  name: string;
  parent_id?: number | null;
  path: string;
  slug?: string | null;
  effective_slug?: string | null;
  owner: CompactUser;
  ticket_count: number;
  created_at: string;
  updated_at: string;
}

export interface CreateComponentRequest {
  name: string;
  parent_id?: number | null;
  slug?: string | null;
  owner_id: number;
}

export interface UpdateComponentRequest {
  name?: string;
  parent_id?: number | null;
  slug?: string | null;
  owner_id?: number;
}

// --- Milestones ---

export interface MilestoneStats {
  total: number;
  new: number;
  in_progress: number;
  verify: number;
  done: number;
  estimated_hours: number;
  remaining_hours: number;
}

export interface Milestone {
  id: number;
  name: string;
  description?: string | null;
  due_date?: string | null;
  status: MilestoneStatus;
  stats: MilestoneStats;
  created_at: string;
  updated_at: string;
}

export interface CreateMilestoneRequest {
  name: string;
  description?: string;
  due_date?: string;
  status?: MilestoneStatus;
}

export interface UpdateMilestoneRequest {
  name?: string;
  description?: string | null;
  due_date?: string | null;
  status?: MilestoneStatus;
}

// --- Users ---

export interface User {
  id: number;
  login: string;
  display_name: string;
  email: string;
  role: UserRole;
  is_active: boolean;
  has_password: boolean;
  has_oidc: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateUserRequest {
  login: string;
  display_name: string;
  email: string;
  password?: string;
  role?: UserRole;
}

export interface UpdateUserRequest {
  display_name?: string;
  email?: string;
  role?: UserRole;
  is_active?: boolean;
}

export interface SetPasswordRequest {
  current_password?: string;
  new_password: string;
}

// --- Auth ---

export interface LoginRequest {
  login: string;
  password: string;
}

export interface AuthUser {
  id: number;
  login: string;
  display_name: string;
  email: string;
  role: UserRole;
}

// --- Attachments ---

export interface Attachment {
  id: number;
  original_name: string;
  mime_type: string;
  size_bytes: number;
  url: string;
}

// --- Mute ---

export interface MuteStatus {
  muted: boolean;
}

// --- SSE Events ---

export type SSEEventType =
  | 'ticket_created'
  | 'ticket_updated'
  | 'comment_created'
  | 'comment_updated'
  | 'comment_deleted';

// --- List wrappers ---

export interface ListResponse<T> {
  items: T[];
}
