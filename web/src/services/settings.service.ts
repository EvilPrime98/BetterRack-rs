import type { IAppSettings } from '../settings.types';
import { API_URL } from './library.service';
import { authHeaders } from './server-config.service';
import { invalidateDirectories } from './fs.service';

async function parseSettingsResponse(response: Response): Promise<IAppSettings> {
    const data = await response.json();
    if (!response.ok) throw new Error(data.message || 'Request failed');
    return data;
}

async function parseSettingsMutationResponse(response: Response): Promise<IAppSettings> {
    const settings = await parseSettingsResponse(response);
    invalidateDirectories();
    return settings;
}

export async function getSettings(): Promise<IAppSettings> {
    const response = await fetch(`${API_URL}/api/settings`, { headers: authHeaders() });
    return parseSettingsResponse(response);
}

export async function updateSettings(
    partial: Partial<Omit<IAppSettings, 'outputDirs'>>,
): Promise<IAppSettings> {
    const response = await fetch(`${API_URL}/api/settings`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify(partial),
    });
    return parseSettingsMutationResponse(response);
}

export async function addLibraryFolder(path: string): Promise<IAppSettings> {
    const response = await fetch(`${API_URL}/api/settings/library-folder`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ path }),
    });
    return parseSettingsMutationResponse(response);
}

export async function removeLibraryFolder(path: string): Promise<IAppSettings> {
    const response = await fetch(`${API_URL}/api/settings/library-folder`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ path }),
    });
    return parseSettingsMutationResponse(response);
}
