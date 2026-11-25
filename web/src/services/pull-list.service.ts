import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';
import type { IPullEntry } from '../store.types';

async function parse<T>(response: Response): Promise<T> {
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(data.message || 'Request failed');
    return data;
}

const jsonHeaders = () => ({ 'Content-Type': 'application/json', ...authHeaders() });
const url = (id?: string) =>
    `${API_URL}/api/pull-list${id === undefined ? '' : `/${encodeURIComponent(id)}`}`;

export async function getPullList(): Promise<IPullEntry[]> {
    return parse(await fetch(url(), { headers: authHeaders() }));
}

export async function addPullEntry(keyword: string): Promise<IPullEntry> {
    return parse(
        await fetch(url(), {
            method: 'POST',
            headers: jsonHeaders(),
            body: JSON.stringify({ keyword, lastSeenId: 0 }),
        }),
    );
}

export async function patchPullEntry(
    id: string,
    patch: { keyword?: string; lastSeenId?: number },
): Promise<IPullEntry> {
    return parse(
        await fetch(url(id), {
            method: 'PATCH',
            headers: jsonHeaders(),
            body: JSON.stringify(patch),
        }),
    );
}

export async function deletePullEntry(id: string): Promise<void> {
    await parse(await fetch(url(id), { method: 'DELETE', headers: authHeaders() }));
}
