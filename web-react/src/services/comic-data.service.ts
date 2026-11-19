import type { IComicLSCache } from '../library.types';
import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';

export async function getComicData(): Promise<Record<string, IComicLSCache>> {
    const response = await fetch(`${API_URL}/api/comic-data`, { headers: authHeaders() });
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to load comic data.');
    return data;
}

export async function updateComicData(
    uid: string,
    partial: Partial<IComicLSCache>,
): Promise<IComicLSCache> {
    const response = await fetch(`${API_URL}/api/comic-data/${uid}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify(partial),
    });
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to update comic data.');
    return data;
}
