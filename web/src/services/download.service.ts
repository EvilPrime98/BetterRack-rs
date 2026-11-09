import { Directory, Filesystem } from '@capacitor/filesystem';
import { API_URL, authHeaders, isAndroidPlatform, withAuthQuery } from './server-config.service';

const ANDROID_FOLDER = 'BetterRack';

function streamUrl(uid: string): string {
    return `${API_URL}/api/stream/${encodeURIComponent(uid)}`;
}

function safeFileName(name: string): string {
    return name.replace(/[/:*?"<>|]/g, '_');
}

export async function downloadComic(uid: string, name: string): Promise<string | undefined> {
    if (isAndroidPlatform()) {
        const path = `${ANDROID_FOLDER}/${safeFileName(name)}`;
        await Filesystem.downloadFile({
            url: streamUrl(uid),
            path,
            directory: Directory.Documents,
            headers: authHeaders(),
            recursive: true,
        });
        return `Documents/${path}`;
    }

    if (typeof window.desktop?.downloadComic === 'function') {
        const url = new URL(streamUrl(uid), window.location.origin).href;
        const savedPath = await window.desktop.downloadComic({
            url,
            name: safeFileName(name),
            headers: authHeaders(),
        });
        return savedPath ?? undefined;
    }

    const link = document.createElement('a');
    link.href = withAuthQuery(streamUrl(uid));
    link.download = name;
    document.body.appendChild(link);
    link.click();
    link.remove();
    return undefined;
}
