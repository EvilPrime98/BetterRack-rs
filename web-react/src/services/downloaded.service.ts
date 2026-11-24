export interface IDownloaded {
    ids: number[];
    paths: Record<string, string>;
}

export async function getDownloaded(): Promise<IDownloaded> {
    const response = await fetch(`/api/downloaded`);
    if (!response.ok) throw new Error('Failed to load downloaded items.');
    return response.json();
}
