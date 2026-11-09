import { API_URL } from './library.service';
import { authHeaders, withAuthQuery } from './server-config.service';
import type { IStoreLink, IStorePost, TStoreProgressEvent, TStoreStrat } from '../store.types';

async function parseJsonResponse<T>(response: Response): Promise<T> {
    const data = await response.json();
    if (!response.ok) throw new Error(data.message || 'Request failed');
    return data;
}

export async function searchComics(params: {
    search: string;
    page?: number;
    perPage?: number;
    exact?: boolean;
}): Promise<IStorePost[]> {
    const searchParams = new URLSearchParams({ search: params.search });
    if (params.page) searchParams.set('page', String(params.page));
    if (params.perPage) searchParams.set('perPage', String(params.perPage));
    if (params.exact) searchParams.set('exact', 'true');
    const response = await fetch(`${API_URL}/api/comics?${searchParams}`, {
        headers: authHeaders(),
    });
    return parseJsonResponse(response);
}

export async function getLatestComics(params?: {
    page?: number;
    perPage?: number;
}): Promise<IStorePost[]> {
    const searchParams = new URLSearchParams({ latest: 'true' });
    if (params?.page) searchParams.set('page', String(params.page));
    if (params?.perPage) searchParams.set('perPage', String(params.perPage));
    const response = await fetch(`${API_URL}/api/comics?${searchParams}`, {
        headers: authHeaders(),
    });
    return parseJsonResponse(response);
}

export async function getComicLinks(id: number, strat: TStoreStrat = 'all'): Promise<IStoreLink[]> {
    const response = await fetch(`${API_URL}/api/comics/${id}/links?strat=${strat}`, {
        headers: authHeaders(),
    });
    const data = await parseJsonResponse<{ error: boolean; message: string; links: IStoreLink[] }>(
        response,
    );
    return data.links;
}

export async function downloadComic({
    id,
    title,
    uuid,
    outputDir,
    strat,
    onProgress,
}: {
    id: number;
    title: string;
    uuid: string;
    outputDir: string;
    strat?: TStoreStrat;
    onProgress: (event: TStoreProgressEvent) => void;
}): Promise<void> {
    const searchParams = new URLSearchParams({ id: String(id), title, uuid, outputDir });
    if (strat) searchParams.set('strat', strat);

    const response = await fetch(`${API_URL}/api/downloads?${searchParams}`, {
        headers: authHeaders(),
    });
    const { jobId } = await parseJsonResponse<{ error: boolean; jobId: string; state: string }>(
        response,
    );

    return new Promise((resolve, reject) => {
        const es = new EventSource(withAuthQuery(`${API_URL}/api/downloads/${jobId}/stream`));

        es.onmessage = (e) => {
            const { progress }: { progress?: TStoreProgressEvent } = JSON.parse(e.data);
            if (!progress) return;
            onProgress(progress);
            if (progress.type === 'done') {
                es.close();
                resolve();
            } else if (progress.type === 'error') {
                es.close();
                reject(new Error(progress.message));
            }
        };

        es.onerror = () => {
            es.close();
            reject(new Error('Lost connection to the server while downloading.'));
        };
    });
}

export type TJobStatus = {
    jobId: string;
    state: 'queued' | 'running' | 'done' | 'error';
    label: string;
    progress?: TStoreProgressEvent;
    pageUrl?: string;
};

export async function getDownloadJobs(): Promise<TJobStatus[]> {
    const response = await fetch(`${API_URL}/api/downloads/jobs`, { headers: authHeaders() });
    const data = await parseJsonResponse<{ error: boolean; jobs: TJobStatus[] }>(response);
    return data.jobs;
}

export async function retryDownloadJob(jobId: string): Promise<{ jobId: string; state: string }> {
    const response = await fetch(`${API_URL}/api/downloads/${jobId}/retry`, {
        method: 'POST',
        headers: authHeaders(),
    });
    return parseJsonResponse<{ error: boolean; jobId: string; state: string }>(response);
}

export async function cancelDownloadJob(jobId: string): Promise<{ jobId: string }> {
    const response = await fetch(`${API_URL}/api/downloads/${jobId}`, {
        method: 'DELETE',
        headers: authHeaders(),
    });
    return parseJsonResponse<{ error: boolean; jobId: string }>(response);
}

export async function startDownloadJob({
    id,
    title,
    uuid,
    outputDir,
    strat,
}: {
    id: number;
    title: string;
    uuid: string;
    outputDir: string;
    strat?: TStoreStrat;
}): Promise<{ jobId: string; state: string }> {
    const response = await fetch(`${API_URL}/api/downloads`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...authHeaders() },
        body: JSON.stringify({ id, title, uuid, outputDir, strat }),
    });

    return parseJsonResponse<{ error: boolean; jobId: string; state: string }>(response);
}
