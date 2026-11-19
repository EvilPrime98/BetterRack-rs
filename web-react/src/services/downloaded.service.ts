import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';

export interface IDownloaded {
    ids: number[];
    paths: Record<string, string>;
}

export async function getDownloaded(): Promise<IDownloaded> {
    const response = await fetch(`${API_URL}/api/downloaded`, { headers: authHeaders() });
    if (!response.ok) throw new Error('Failed to load downloaded items.');
    return response.json();
}
