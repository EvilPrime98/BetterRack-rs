import { useEffect, useRef, useState } from 'react';
import { Layout } from '@/layout';
import {
    cancelDownloadJob,
    getDownloadJobs,
    retryDownloadJob,
    type TJobStatus,
} from '@/services/store.service';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { useLibraryStore } from '@/stores/library.store';
import { useDownloadedStore } from '@/stores/downloaded.store';
import { useConfirmModalStore } from '@/stores/confirmModal.store';
import { DOWNLOAD_TRACKER } from '@/services/download-guard.service';
import { countActiveJobs } from '@/lib/active-downloads';
import { toast } from '@/services/toast.service';
import { POLL_INTERVAL_MS } from '@/data';
import styles from './store-downloads.page.module.css';
import {
    JobRow,
    type TPendingAction,
    type TPendingJobs,
} from '@/components/downloads-page/job-row';
import type { TJobStates } from '@/store.types';
import { useUserPrefStore } from '@/stores/userPref.store';

export function StoreDownloadsPage() {
    const setTitle = useDocumentTitleStore((s) => s.setTitle);
    const [jobs, setJobs] = useState<TJobStatus[]>([]);
    const [error, setError] = useState('');
    const [loaded, setLoaded] = useState(false);
    const [pending, setPending] = useState<TPendingJobs>(new Map());
    const pendingRef = useRef<TPendingJobs>(pending);

    function stateByJobId(jobs: TJobStatus[]): TJobStates {
        return new Map(jobs.map((job) => [job.jobId, job.state]));
    }

    function findNewlyDone(previous: TJobStates, jobs: TJobStatus[]): TJobStatus[] {
        return jobs.filter((job) => {
            const before = previous.get(job.jobId);
            return job.state === 'done' && before !== undefined && before !== 'done';
        });
    }

    async function notifyFinished(finished: TJobStatus[]) {
        if (finished.length === 0) return;
        finished.forEach((job) => toast.success(`${job.label} downloaded`));
        await useDownloadedStore.getState().load();
        await useLibraryStore.getState().refreshLibrary({ silent: true });
    }

    async function withPending(jobId: string, action: TPendingAction, run: () => Promise<void>) {
        if (pendingRef.current.has(jobId)) return;
        pendingRef.current = new Map(pendingRef.current).set(jobId, action);
        setPending(pendingRef.current);
        try {
            await run();
        } finally {
            const next = new Map(pendingRef.current);
            next.delete(jobId);
            pendingRef.current = next;
            setPending(next);
        }
    }

    function handleRetry(jobId: string) {
        return withPending(jobId, 'retrying', async () => {
            await retryDownloadJob(jobId);
            setJobs(await getDownloadJobs());
        });
    }

    async function handleStop(job: TJobStatus) {
        if (useUserPrefStore.getState().pref.askStopDownloads === true) {
            const confirmed = await useConfirmModalStore.getState().confirmDialog({
                title: 'Stop this download?',
                message: `"${job.label}" will be stopped and removed from the list.`,
                confirmLabel: 'Stop download',
                onDontAskAgain: () =>
                    useUserPrefStore.getState().setPref({ askStopDownloads: false }),
            });
            if (!confirmed) return;
        }
        await withPending(job.jobId, 'stopping', async () => {
            try {
                await cancelDownloadJob(job.jobId);
                setJobs(await getDownloadJobs());
            } catch (e) {
                toast.error(e instanceof Error ? e.message : 'Failed to stop the download.');
            }
        });
    }

    function handleDelete(jobId: string) {
        return withPending(jobId, 'deleting', async () => {
            try {
                await cancelDownloadJob(jobId);
                setJobs(await getDownloadJobs());
            } catch (e) {
                toast.error(e instanceof Error ? e.message : 'Failed to delete the download.');
            }
        });
    }

    useEffect(() => {
        setTitle('Downloads');
    }, [setTitle]);

    useEffect(() => {
        let active = true;
        let previousStates: TJobStates = new Map();

        async function load() {
            try {
                const next = await getDownloadJobs();
                if (!active) return;
                const finished = findNewlyDone(previousStates, next);
                previousStates = stateByJobId(next);
                DOWNLOAD_TRACKER.set(countActiveJobs(next));
                setJobs(next);
                setError('');
                notifyFinished(finished);
            } catch (e) {
                if (!active) return;
                setError(e instanceof Error ? e.message : 'Failed to load download jobs.');
            } finally {
                if (active) setLoaded(true);
            }
        }

        load();
        const timer = setInterval(load, POLL_INTERVAL_MS);

        return () => {
            active = false;
            clearInterval(timer);
        };
    }, []);

    return (
        <Layout>
            <section className={styles.page}>
                <header className={styles.header}>
                    <span className={styles.eyebrow}>Store</span>
                    <h1 className={styles.title}>Downloads</h1>
                </header>

                {jobs.length === 0 ? (
                    <p className={styles.empty}>
                        {error ? error : loaded ? 'No active downloads.' : 'Loading downloads…'}
                    </p>
                ) : (
                    <ul className={styles.list}>
                        {jobs.map((job) => (
                            <JobRow
                                key={job.jobId}
                                job={job}
                                onRetry={handleRetry}
                                onStop={handleStop}
                                onDelete={handleDelete}
                                pending={pending.get(job.jobId)}
                            />
                        ))}
                    </ul>
                )}
            </section>
        </Layout>
    );
}
