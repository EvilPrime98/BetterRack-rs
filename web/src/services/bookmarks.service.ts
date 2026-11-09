import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';
import type { IStoreBookmark, IStorePost } from '../store.types';

async function parse<T>(response: Response): Promise<T> {
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(data.message || 'Request failed');
    return data;
}

const jsonHeaders = () => ({ 'Content-Type': 'application/json', ...authHeaders() });
const url = (key?: string) =>
    `${API_URL}/api/bookmarks${key === undefined ? '' : `/${encodeURIComponent(key)}`}`;

export async function getBookmarks(): Promise<IStoreBookmark[]> {
    return parse(await fetch(url(), { headers: authHeaders() }));
}

export async function addBookmark(post: IStorePost, folder?: string): Promise<IStoreBookmark> {
    return parse(
        await fetch(url(), {
            method: 'POST',
            headers: jsonHeaders(),
            body: JSON.stringify({ post, folder: folder ?? null }),
        }),
    );
}

export async function patchBookmark(
    key: string,
    patch: { folder?: string | null; rating?: number; read?: boolean },
): Promise<IStoreBookmark> {
    return parse(
        await fetch(url(key), {
            method: 'PATCH',
            headers: jsonHeaders(),
            body: JSON.stringify(patch),
        }),
    );
}

export async function deleteBookmark(key: string): Promise<void> {
    await parse(await fetch(url(key), { method: 'DELETE', headers: authHeaders() }));
}
