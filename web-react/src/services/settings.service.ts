import type { IAppSettings } from '../settings.types';
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
    const response = await fetch(`/api/settings`);
    return parseSettingsResponse(response);
}

export async function updateSettings(
    partial: Partial<Omit<IAppSettings, 'outputDirs'>>,
): Promise<IAppSettings> {
    const response = await fetch(`/api/settings`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(partial),
    });
    return parseSettingsMutationResponse(response);
}

export async function addLibraryFolder(path: string): Promise<IAppSettings> {
    const response = await fetch(`/api/settings/library-folder`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path }),
    });
    return parseSettingsMutationResponse(response);
}

export async function removeLibraryFolder(path: string): Promise<IAppSettings> {
    const response = await fetch(`/api/settings/library-folder`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path }),
    });
    return parseSettingsMutationResponse(response);
}
