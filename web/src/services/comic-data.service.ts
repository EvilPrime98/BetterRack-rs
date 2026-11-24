import type { IComicIdentity, IComicLSCache } from '../library.types';

export async function getComicData(): Promise<Record<string, IComicLSCache>> {
    const response = await fetch(`/api/comic-data`);
    return await response.json();
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
    return await response.json();
}

export async function updateComicIdentity(
    uid: string,
    partial: Partial<IComicIdentity>,
): Promise<IComicIdentity> {
    const response = await fetch(`/api/comic-data/${uid}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(partial),
    });
    return await response.json();
}
