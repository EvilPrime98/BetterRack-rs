import type { WikiComic } from 'better-wiki';
import { DEFAULT_IMAGE_SIZE } from '../data';
import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';

export async function fetchComics(
    title: string,
    thumbnailSize: number = DEFAULT_IMAGE_SIZE,
): Promise<WikiComic[]> {
    const params = new URLSearchParams({ title, thumbnailSize: String(thumbnailSize) });
    const response = await fetch(`${API_URL}/api/wiki/comics?${params}`, {
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to search the wiki.');
    return data;
}

export async function fetchComicById(id: number, sourceWiki: string): Promise<WikiComic> {
    const params = new URLSearchParams({ sourceWiki });
    const response = await fetch(`${API_URL}/api/wiki/comic/${id}?${params.toString()}`, {
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to load wiki comic.');
    return data;
}
