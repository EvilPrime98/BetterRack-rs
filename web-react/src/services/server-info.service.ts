import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';

export interface IServerUpdateInfo {
    current: string;
    latest: string | null;
    updateAvailable: boolean;
    url: string | null;
}

async function getJson<T>(path: string): Promise<T> {
    const response = await fetch(`${API_URL}${path}`, { headers: authHeaders() });
    const data = await response.json();
    if (!response.ok) throw new Error(data.message || 'Request failed');
    return data;
}

export async function getServerVersion(): Promise<string> {
    return (await getJson<{ version: string }>('/api/server/info')).version;
}

export function checkServerUpdate(): Promise<IServerUpdateInfo> {
    return getJson('/api/server/update');
}

export async function getServerLogs(lines = 200): Promise<string[]> {
    return (await getJson<{ lines: string[] }>(`/api/server/logs?lines=${lines}`)).lines;
}

export function describeUpdate(info: IServerUpdateInfo): string {
    if (info.updateAvailable) return `Version ${info.latest} is available.`;
    if (info.latest) return `Up to date (${info.current}).`;
    return 'Could not check for updates.';
}
