import type { IComicLSCache } from '../library.types';

export async function getComicData(): Promise<Record<string, IComicLSCache>> {
    const response = await fetch(`/api/comic-data`);
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to load comic data.');
    return data;
}

export async function updateComicData(
    uid: string,
    partial: Partial<IComicLSCache>,
): Promise<IComicLSCache> {
    const response = await fetch(`/api/comic-data/${uid}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(partial),
    });
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to update comic data.');
    return data;
}
