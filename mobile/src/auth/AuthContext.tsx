import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { getAccessToken } from '../utils/storage';
import { login as apiLogin, register as apiRegister, logout as apiLogout, getAuthStatus } from '../api/auth';
import { setOnUnauthorized } from '../api/client';
import type { LoginRequest, RegisterRequest } from '../api/types';

interface AuthContextType {
  isAuthenticated: boolean;
  isLoading: boolean;
  userId: number | null;
  login: (params: LoginRequest) => Promise<void>;
  register: (params: RegisterRequest) => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [userId, setUserId] = useState<number | null>(null);

  const handleLogout = useCallback(async () => {
    await apiLogout();
    setIsAuthenticated(false);
    setUserId(null);
  }, []);

  useEffect(() => {
    // Set up unauthorized callback
    setOnUnauthorized(() => {
      setIsAuthenticated(false);
      setUserId(null);
    });

    // Check existing token on mount
    async function checkAuth() {
      try {
        const token = await getAccessToken();
        if (token) {
          const status = await getAuthStatus();
          setIsAuthenticated(status.authenticated);
          setUserId(status.user_id);
        }
      } catch {
        setIsAuthenticated(false);
        setUserId(null);
      } finally {
        setIsLoading(false);
      }
    }
    checkAuth();
  }, []);

  const login = useCallback(async (params: LoginRequest) => {
    const response = await apiLogin(params);
    setIsAuthenticated(true);
    // Decode user_id from token or use auth status
    try {
      const status = await getAuthStatus();
      setUserId(status.user_id);
    } catch {
      // Token is valid since login succeeded
      setIsAuthenticated(true);
    }
  }, []);

  const register = useCallback(async (params: RegisterRequest) => {
    await apiRegister(params);
    setIsAuthenticated(true);
    try {
      const status = await getAuthStatus();
      setUserId(status.user_id);
    } catch {
      setIsAuthenticated(true);
    }
  }, []);

  return (
    <AuthContext.Provider
      value={{ isAuthenticated, isLoading, userId, login, register, logout: handleLogout }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
