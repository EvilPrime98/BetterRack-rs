import type { TJobStatus } from '../services/store.service';

export function countActiveJobs(jobs: Pick<TJobStatus, 'state'>[]): number {
    return jobs.filter((job) => job.state === 'queued' || job.state === 'running').length;
}

export function createDownloadTracker(
    fetchJobs: () => Promise<Pick<TJobStatus, 'state'>[]>,
    intervalMs = 3000,
) {
    let count = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const poll = async () => {
        timer = undefined;
        try {
            count = countActiveJobs(await fetchJobs());
        } catch {
            count = 0;
        }
        if (count > 0) timer = setTimeout(poll, intervalMs);
    };

    return {
        count: () => count,
        set(next: number) {
            count = next;
        },
        started() {
            count = Math.max(count, 1);
            if (!timer) timer = setTimeout(poll, intervalMs);
        },
        stop() {
            clearTimeout(timer);
            timer = undefined;
        },
    };
}
