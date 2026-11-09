import type { TJobStatus } from '@/services/store.service';
import type { TStoreProgressEvent } from '@/store.types';
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

export function JobRow(
    job: TJobStatus,
    retryingIds: Set<string>,
    stoppingIds: Set<string>,
): string {
    const state = job.state;
    const percent = percentFor(job.progress);
    const detail = detailFor(job.progress);

    const bar =
        percent === null
            ? ''
            : `<div class="${styles.progress}">
               <span class="${styles.progressFill}" style="width:${percent}%"></span>
           </div>
           <span class="${styles.percent}">${percent}%</span>`;

    const retry =
        state === 'error'
            ? `<button type="button" class="${styles.retry}" data-retry-job-id="${job.jobId}"${retryingIds.has(job.jobId) ? ' disabled' : ''}>Retry</button>`
            : '';

    const stop = canStop(job)
        ? `<button type="button" class="${styles.stop}" data-stop-job-id="${job.jobId}"${stoppingIds.has(job.jobId) ? ' disabled' : ''}>Stop</button>`
        : '';

    return `<li class="${styles.row}" data-state="${state}">
        <div class="${styles.rowHead}">
            <span class="${styles.label}">${job.label}</span>
            <span class="${styles.badge}" data-state="${state}">${STATE_LABEL[state]}</span>
        </div>
        ${bar}
        ${
            detail || job.pageUrl
                ? `<div class="${styles.detailRow}">
            ${detail ? `<span class="${styles.detail}">${detail}</span>` : ''}
            ${job.pageUrl ? `<a class="${styles.pageLink}" data-external-url="${job.pageUrl}" href="${job.pageUrl}" target="_blank" rel="noopener noreferrer">Open comic page</a>` : ''}
        </div>`
                : ''
        }
        ${retry}
        ${stop}
    </li>`;
}
