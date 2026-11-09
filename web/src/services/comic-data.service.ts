import type { IComicIdentity, IComicLSCache } from '../library.types';
import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';

export async function getComicData(): Promise<Record<string, IComicLSCache>> {
    const response = await fetch(`${API_URL}/api/comic-data`, { headers: authHeaders() });
    return await response.json();
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
    return await response.json();
}

export async function updateComicIdentity(
    uid: string,
    partial: Partial<IComicIdentity>,
): Promise<IComicIdentity> {
    const response = await fetch(`${API_URL}/api/comic-data/${uid}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify(partial),
    });
    return await response.json();
}
