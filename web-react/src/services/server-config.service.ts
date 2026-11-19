import { Capacitor } from '@capacitor/core';

const STORAGE_KEY = 'br_server_url';
const REMOTE_MODE_KEY = 'br_remote_mode';
const API_KEY_STORAGE_KEY = 'br_server_api_key';

export function isAndroidPlatform(): boolean {
    return Capacitor.getPlatform() === 'android';
}

export function isRemoteModeEnabled(): boolean {
    return localStorage.getItem(REMOTE_MODE_KEY) === '1';
}

export function getStoredServerUrl(): string {
    return localStorage.getItem(STORAGE_KEY) ?? '';
}

export function getStoredApiKey(): string {
    return localStorage.getItem(API_KEY_STORAGE_KEY) ?? '';
}

export function needsServerSetup(): boolean {
    return (isAndroidPlatform() || isRemoteModeEnabled()) && !getStoredServerUrl();
}

function normalizeServerUrl(rawUrl: string): string {
    let url = rawUrl.trim();
    if (!/^https?:\/\//i.test(url)) url = `http://${url}`;
    return url.replace(/\/+$/, '');
}

export function setStoredApiKey(apiKey: string): void {
    if (apiKey) localStorage.setItem(API_KEY_STORAGE_KEY, apiKey);
    else localStorage.removeItem(API_KEY_STORAGE_KEY);
}

export function setServerUrl(rawUrl: string): void {
    const normalized = normalizeServerUrl(rawUrl);
    localStorage.setItem(STORAGE_KEY, normalized);
    API_URL = normalized;
}

/** Opt-in path for non-Android clients: pairs a server URL with the remote-mode flag and an optional API key. */
export function setRemoteServer(rawUrl: string, apiKey: string): void {
    setServerUrl(rawUrl);
    localStorage.setItem(REMOTE_MODE_KEY, '1');
    setStoredApiKey(apiKey);
}

export function clearRemoteServer(): void {
    localStorage.removeItem(STORAGE_KEY);
    localStorage.removeItem(REMOTE_MODE_KEY);
    localStorage.removeItem(API_KEY_STORAGE_KEY);
}

function resolveInitialApiUrl(): string {
    if (isAndroidPlatform() || isRemoteModeEnabled()) return getStoredServerUrl();
    return import.meta.env.VITE_API_URL ?? '';
}

export let API_URL = resolveInitialApiUrl();

const API_KEY_HEADER = 'x-br-api-key';
const API_KEY_QUERY_PARAM = 'key';

export function authHeaders(): Record<string, string> {
    const key = getStoredApiKey();
    return key ? { [API_KEY_HEADER]: key } : {};
}

/** Add the API key as a query parameter. Use this for URLs that cannot carry a custom header, such as <img src> and EventSource. */
export function withAuthQuery(url: string): string {
    const key = getStoredApiKey();
    if (!key) return url;
    const separator = url.includes('?') ? '&' : '?';
    return `${url}${separator}${API_KEY_QUERY_PARAM}=${encodeURIComponent(key)}`;
}
