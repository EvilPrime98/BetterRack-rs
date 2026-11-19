import type { TJobStatus } from '@/services/store.service';
import type { TStoreProgressEvent } from '@/store.types';
import { ExternalLink } from '@/components/external-link/external-link';
import styles from '@/pages/store-downloads.page.module.css';

function percentFor(progress?: TStoreProgressEvent): number | null {
    if (!progress) return null;
    if (progress.type === 'progress') return Math.round(progress.percent);
    if (progress.type === 'extracting') {
        return progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : null;
    }
    return null;
}

function detailFor(progress?: TStoreProgressEvent): string {
    if (!progress) return '';
    switch (progress.type) {
        case 'preparing':
            return 'Preparing…';
        case 'retrying':
            return progress.status
                ? `Retrying after HTTP ${progress.status} — waiting ${progress.delaySec}s`
                : `Connection lost — retrying in ${progress.delaySec}s`;
        case 'progress':
            return `${progress.receivedMB} / ${progress.totalMB} MB`;
        case 'extracting':
            return `Extracting ${progress.done} / ${progress.total}`;
        case 'done':
            return progress.filename;
        case 'error':
            return progress.message;
    }
}

function canStop(job: TJobStatus): boolean {
    const isActive = job.state === 'queued' || job.state === 'running';
    return isActive && job.progress?.type !== 'extracting';
}

const STATE_LABEL: Record<TJobStatus['state'], string> = {
    queued: 'Queued',
    running: 'Downloading',
    done: 'Done',
    error: 'Failed',
};

export type TPendingAction = 'retrying' | 'stopping' | 'deleting';
export type TPendingJobs = Map<string, TPendingAction>;

export function JobRow({
    job,
    onRetry,
    onStop,
    onDelete,
    pending,
}: {
    job: TJobStatus;
    onRetry: (jobId: string) => void;
    onStop: (job: TJobStatus) => void;
    onDelete: (jobId: string) => void;
    pending?: TPendingAction;
}) {
    const percent = percentFor(job.progress);
    const detail = detailFor(job.progress);

    return (
        <li className={styles.row} data-state={job.state}>
            <div className={styles.rowHead}>
                <span className={styles.label}>{job.label}</span>
                <span className={styles.badge} data-state={job.state}>
                    {STATE_LABEL[job.state]}
                </span>
            </div>

            {percent !== null && (
                <>
                    <div className={styles.progress}>
                        <span className={styles.progressFill} style={{ width: `${percent}%` }} />
                    </div>
                    <span className={styles.percent}>{percent}%</span>
                </>
            )}

            {detail && (
                <div className={styles.detailRow}>
                    <span className={styles.detail}>{detail}</span>
                </div>
            )}

            {(job.state === 'error' || job.state === 'done') && (
                <div className={styles.actions}>
                    {job.state === 'error' && (
                        <button
                            type="button"
                            className={styles.stop}
                            disabled={pending === 'retrying'}
                            onClick={() => onRetry(job.jobId)}
                        >
                            Retry
                        </button>
                    )}

                    <button
                        type="button"
                        className={styles.stop}
                        disabled={pending === 'deleting'}
                        onClick={() => onDelete(job.jobId)}
                    >
                        Delete
                    </button>
                </div>
            )}

            {job.state === 'error' && job.pageUrl && /^https?:\/\//.test(job.pageUrl) && (
                <div className={styles.actions}>
                    <ExternalLink className={styles.pageLink} href={job.pageUrl}>
                        Open comic page
                    </ExternalLink>
                </div>
            )}

            {canStop(job) && (
                <div className={styles.actions}>
                    <button
                        type="button"
                        className={styles.stop}
                        disabled={pending === 'stopping'}
                        onClick={() => onStop(job)}
                    >
                        Stop
                    </button>
                </div>
            )}
        </li>
    );
}
