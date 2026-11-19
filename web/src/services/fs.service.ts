import { API_URL, authHeaders } from './server-config.service';

let cachedDirectories: string[] | null = null;
let inFlightRefresh: Promise<string[]> | null = null;
let cacheGeneration = 0;

async function fetchDirectories(): Promise<string[]> {
    const response = await fetch(`${API_URL}/api/directories`, { headers: authHeaders() });
    const data = await response.json();
    if (!response.ok) throw new Error(data.message || 'Failed to load directories.');
    return data.directories ?? [];
}

export function getCachedDirectories(): string[] | null {
    return cachedDirectories;
}

export function refreshDirectories(): Promise<string[]> {
    if (inFlightRefresh) return inFlightRefresh;
    const requestGeneration = cacheGeneration;
    const request = fetchDirectories()
        .then((dirs) => {
            if (requestGeneration === cacheGeneration) cachedDirectories = dirs;
            return dirs;
        })
        .finally(() => {
            if (inFlightRefresh === request) inFlightRefresh = null;
        });
    inFlightRefresh = request;
    return request;
}

export function invalidateDirectories(): void {
    cacheGeneration++;
    cachedDirectories = null;
    inFlightRefresh = null;
}

export function areDirectoryListsEqual(a: string[], b: string[]): boolean {
    return a.length === b.length && a.every((dir, index) => dir === b[index]);
}
