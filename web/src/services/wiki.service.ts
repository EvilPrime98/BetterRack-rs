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
    return await response.json();
}
