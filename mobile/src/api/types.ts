// Auth
export interface LoginRequest {
  email: string;
  password: string;
}

export interface RegisterRequest {
  email: string;
  password: string;
  phone_number: string;
}

export interface AuthResponse {
  message: string;
  token: string;
  refresh_token: string;
}

export interface AuthStatus {
  authenticated: boolean;
  user_id: number;
  is_admin: boolean;
}

// Chat
export interface ChatRequest {
  message: string;
}

export interface MediaResult {
  video_id: string;
  title: string;
  thumbnail: string;
  duration: string;
  channel: string | null;
}

export interface ChatResponse {
  message: string;
  credits_charged: number;
  media: MediaResult[] | null;
  created_task_id: number | null;
}

// Dashboard
export interface DashboardTask {
  id: number;
  title: string;
  due_date: string | null;
  completed: boolean;
}

export interface DashboardSummary {
  attention_count: number;
  upcoming_tasks: DashboardTask[];
  quiet_mode: boolean;
  quiet_mode_until: string | null;
  upcoming_digests: string[];
}

// Triage
export interface TriageItem {
  id: number;
  item_type: string;
  source: string;
  summary: string;
  sender: string | null;
  created_at: number;
  suggested_response: string | null;
}

export interface TriageResponse {
  items: TriageItem[];
}

// Profile
export interface UserProfile {
  email: string;
  phone_number: string;
  timezone: string | null;
  credits_left: number;
  sub_tier: string | null;
  plan_type: string | null;
  language: string | null;
}

// Push token
export interface PushTokenRequest {
  platform: string;
  token: string;
}
