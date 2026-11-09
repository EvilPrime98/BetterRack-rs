import type {
    IReadResponse,
    IBookmarksResponse,
    ILibraryIndexGroup,
    ILibraryPage,
    ILibraryRefreshResponse,
    ILibraryResponseItem,
    TIdentifyLibraryStatus,
    IReadingResponse,
    IRecentlyAddedResponse,
} from '../library.types';
import { API_URL, authHeaders } from './server-config.service';
import { invalidateDirectories } from './fs.service';

export { API_URL };

export const LIBRARY_PAGE_SIZE = 100;

export async function getRecentlyAdded(windowHours?: number): Promise<IRecentlyAddedResponse> {
    const query =
        typeof windowHours === 'number' && windowHours > 0
            ? `?${new URLSearchParams({ windowHours: String(windowHours) })}`
            : '';
    const response = await fetch(`${API_URL}/api/library/recent${query}`, {
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok)
        throw new Error(
            (data as { message?: string })?.message || 'Failed to load recently added comics.',
        );
    return data as IRecentlyAddedResponse;
}

export async function getReading(): Promise<IReadingResponse> {
    const response = await fetch(`${API_URL}/api/library/reading`, { headers: authHeaders() });
    const data = await response.json();
    if (!response.ok)
        throw new Error(
            (data as { message?: string })?.message || 'Failed to load comics in progress.',
        );
    return data as IReadingResponse;
}

export async function getLibraryIndex(): Promise<ILibraryIndexGroup[]> {
    const response = await fetch(`${API_URL}/api/library/index`, { headers: authHeaders() });
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to load library index.');
    return data as ILibraryIndexGroup[];
}

export type TLibraryStructure = 'folders' | 'series';

export async function getLibraryPage({
    limit = LIBRARY_PAGE_SIZE,
    offset = 0,
    structure = 'folders',
}: {
    limit?: number;
    offset?: number;
    structure?: TLibraryStructure;
} = {}): Promise<ILibraryPage> {
    const endpoint = structure === 'series' ? '/api/library/by-series' : '/api/library';
    const params = new URLSearchParams({ limit: String(limit), offset: String(offset) });
    const response = await fetch(`${API_URL}${endpoint}?${params}`, { headers: authHeaders() });
    const data = await response.json();
    if (!response.ok)
        throw new Error((data as { message?: string })?.message || 'Failed to load library.');
    return data as ILibraryPage;
}

export async function refreshLibrary(): Promise<ILibraryRefreshResponse> {
    const response = await fetch(`${API_URL}/api/library/refresh`, { headers: authHeaders() });
    const data: ILibraryRefreshResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data;
}

export async function reader({ uid }: { uid: string }) {
    const response = await fetch(`${API_URL}/read/${uid}`, {
        cache: 'no-store',
        headers: authHeaders(),
    });
    const data: IReadResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    return { pages: data.pages, title: data.title };
}

export async function readerBookmarks({ uid }: { uid: string }) {
    const response = await fetch(`${API_URL}/read/${uid}/bookmarks`, {
        cache: 'no-store',
        headers: authHeaders(),
    });
    const data: IBookmarksResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data.bookmarks;
}

export async function readerRefresh({ uid }: { uid: string }) {
    const response = await fetch(`${API_URL}/read/${uid}/refresh`, {
        cache: 'no-store',
        headers: authHeaders(),
    });
    const data: IReadResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data.pages;
}

export async function deleteFile(fileUid: string): Promise<ILibraryRefreshResponse> {
    const response = await fetch(`${API_URL}/api/library/file`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ fileUid }),
    });
    const data: ILibraryRefreshResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data;
}

export async function deleteFolder(folderUid: string): Promise<ILibraryRefreshResponse> {
    const response = await fetch(`${API_URL}/api/library/folder`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ folderUid }),
    });
    const data: ILibraryRefreshResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    invalidateDirectories();
    return data;
}

export async function createFolder(
    folderName: string,
    parentFolderUid?: string,
): Promise<ILibraryRefreshResponse> {
    const response = await fetch(`${API_URL}/api/library/folder`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ folderName, parentFolderUid }),
    });
    const data: ILibraryRefreshResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    invalidateDirectories();
    return data;
}

export async function identifyLibraryEntry(
    uid: string,
): Promise<Pick<ILibraryResponseItem, 'identified' | 'comic' | 'metaSource'>> {
    const response = await fetch(`${API_URL}/api/library/${uid}/identify`, {
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data;
}

export async function reidentifyFile(
    uid: string,
): Promise<Pick<ILibraryResponseItem, 'identified' | 'comic' | 'metaSource'>> {
    const response = await fetch(`${API_URL}/api/library/${uid}/identify/reset`, {
        method: 'POST',
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data;
}

export async function unidentifyFile(fileUid: string): Promise<ILibraryRefreshResponse> {
    const response = await fetch(`${API_URL}/api/library/file/unidentify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ fileUid }),
    });
    const data: ILibraryRefreshResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data;
}

export async function reidentifyAllLibrary(): Promise<{ error: boolean; message: string }> {
    const response = await fetch(`${API_URL}/api/library/identify/reset-all`, {
        method: 'POST',
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.message);
    return data;
}

export async function startIdentifyLibrary(): Promise<TIdentifyLibraryStatus> {
    const response = await fetch(`${API_URL}/api/library/identify/all`, {
        method: 'POST',
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.message ?? 'Failed to start library identification.');
    return data;
}

export async function getIdentifyLibraryStatus(): Promise<TIdentifyLibraryStatus> {
    const response = await fetch(`${API_URL}/api/library/identify/all`, {
        cache: 'no-store',
        headers: authHeaders(),
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.message ?? 'Failed to read identification status.');
    return data;
}

export async function moveFile(
    fileUid: string,
    targetFolderUid?: string,
): Promise<ILibraryRefreshResponse> {
    const response = await fetch(`${API_URL}/api/library/file/move`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ fileUid, targetFolderUid }),
    });
    const data: ILibraryRefreshResponse = await response.json();
    if (!response.ok) throw new Error(data.message);
    invalidateDirectories();
    return data;
}

export async function retryThumbnail(uid: string): Promise<void> {
    const response = await fetch(`${API_URL}/api/thumbnail/${uid}/retry`, {
        method: 'POST',
        headers: authHeaders(),
    });
    if (!response.ok) {
        const data: { message?: string } = await response.json().catch(() => ({}));
        throw new Error(data.message ?? 'Thumbnail generation failed.');
    }
}
