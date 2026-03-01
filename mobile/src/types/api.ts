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

export interface TotpVerifyRequest {
  user_id: number;
  code: string;
}

// Profile
export interface UserProfile {
  id: number;
  email: string;
  phone_number: string;
  timezone: string | null;
  language: string | null;
  quiet_hours_start: string | null;
  quiet_hours_end: string | null;
  notification_preferences: string | null;
}

export interface ProfileUpdateRequest {
  timezone?: string;
  language?: string;
  quiet_hours_start?: string;
  quiet_hours_end?: string;
}

// Dashboard
export interface DashboardSummary {
  total_items: number;
  unread_items: number;
  credits_remaining: number;
  connections_active: number;
}

export interface DashboardItem {
  id: number;
  item_type: string;
  title: string;
  content: string;
  source: string;
  created_at: string;
  is_read: boolean;
}

// Chat
export interface ChatMessage {
  type: "chat" | "chat_response" | "chat_error" | "ping" | "pong";
  message?: string;
  error?: string;
  credits_charged?: number;
  media?: string | null;
  created_task_id?: number | null;
}

// Connections
export interface ConnectionStatus {
  connected: boolean;
  service: string;
  username?: string;
  details?: Record<string, unknown>;
}

export interface QRCodeResponse {
  qr_code: string; // base64 encoded
  session_id?: string;
}

// Billing
export interface CreditsDashboard {
  credits_remaining: number;
  credits_used: number;
  plan: string;
  renewal_date?: string;
}

export interface PricingPlan {
  id: string;
  name: string;
  price: number;
  credits: number;
  features: string[];
}
