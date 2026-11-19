import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';

export async function getDownloadedIds(): Promise<number[]> {
    const response = await fetch(`${API_URL}/api/downloaded`, { headers: authHeaders() });
    if (!response.ok) throw new Error('Failed to load downloaded items.');
    return response.json();
}
