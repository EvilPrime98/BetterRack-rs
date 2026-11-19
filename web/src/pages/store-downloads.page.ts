import { DOWNLOAD_TRACKER } from '../services/download-guard.service';
import { countActiveJobs } from '../lib/active-downloads';
import {
    UltraComponent,
    ultraCompState,
    ultraReplaceChildren,
    type IUltraCompStateStateful,
} from 'ultra-light-js';
import styles from './store-downloads.page.module.css';
import { Browser } from '@capacitor/browser';
import { isAndroidPlatform } from '@/services/server-config.service';
import {
    cancelDownloadJob,
    getDownloadJobs,
    retryDownloadJob,
    type TJobStatus,
} from '@/services/store.service';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { LIBRARY_CONTEXT } from '../context/library.context';
import { CONFIRM_MODAL_CTX } from '../context/confirm-modal.context';
import { toast } from '@/services/toast.service';
import { POLL_INTERVAL_MS } from '@/data';
import type { TJobStates } from '../store.types';
import type { TPendingAction, TPendingJobs } from '@/components/downloads-page/job-row';
import { JobRow } from '@/components/downloads-page/job-row';
import { USER_PREF } from '../context/user-pref-cache.context';

interface IStoreDownloadsState {
    jobs: IUltraCompStateStateful<TJobStatus[]>;
    error: IUltraCompStateStateful<string>;
    loaded: IUltraCompStateStateful<boolean>;
    pending: IUltraCompStateStateful<TPendingJobs>;
    load: () => Promise<void>;
    retry: (jobId: string) => Promise<void>;
    stop: (job: TJobStatus) => Promise<void>;
    remove: (jobId: string) => Promise<void>;
}

function withPending(
    comp: IStoreDownloadsState,
    jobId: string,
    action: TPendingAction,
    run: () => Promise<void>,
): Promise<void> {
    return (async () => {
        if (comp.pending.get().has(jobId)) return;
        comp.pending.set(new Map(comp.pending.get()).set(jobId, action));
        try {
            await run();
        } finally {
            const next = new Map(comp.pending.get());
            next.delete(jobId);
            comp.pending.set(next);
        }
    })();
}

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
    await LIBRARY_CONTEXT.refreshLibrary({ silent: true });
}

export function StoreDownloadsPage() {
    let previousStates: TJobStates = new Map();

    const store: IStoreDownloadsState = ultraCompState({
        jobs: [] as TJobStatus[],
        error: '',
        loaded: false,
        pending: new Map() as TPendingJobs,

        load: async (comp: IStoreDownloadsState) => {
            try {
                const jobs = await getDownloadJobs();
                const finished = findNewlyDone(previousStates, jobs);
                previousStates = stateByJobId(jobs);
                DOWNLOAD_TRACKER.set(countActiveJobs(jobs));
                comp.jobs.set(jobs);
                comp.error.set('');
                notifyFinished(finished);
            } catch (e) {
                comp.error.set(e instanceof Error ? e.message : 'Failed to load download jobs.');
            } finally {
                comp.loaded.set(true);
            }
        },

        retry: async (comp: IStoreDownloadsState, jobId: string) => {
            await withPending(comp, jobId, 'retrying', async () => {
                await retryDownloadJob(jobId);
                await comp.load();
            });
        },

        stop: async (comp: IStoreDownloadsState, job: TJobStatus) => {
            if (USER_PREF.getPref('askStopDownloads') === true) {
                const confirmed = await CONFIRM_MODAL_CTX.confirmDialog({
                    title: 'Stop this download?',
                    message: `"${job.label}" will be stopped and removed from the list.`,
                    confirmLabel: 'Stop download',
                    onDontAskAgain: () => USER_PREF.setPref({ askStopDownloads: false }),
                });
                if (!confirmed) return;
            }
            await withPending(comp, job.jobId, 'stopping', async () => {
                try {
                    await cancelDownloadJob(job.jobId);
                    await comp.load();
                } catch (e) {
                    toast.error(e instanceof Error ? e.message : 'Failed to stop the download.');
                }
            });
        },

        remove: async (comp: IStoreDownloadsState, jobId: string) => {
            await withPending(comp, jobId, 'deleting', async () => {
                try {
                    await cancelDownloadJob(jobId);
                    await comp.load();
                } catch (e) {
                    toast.error(e instanceof Error ? e.message : 'Failed to delete the download.');
                }
            });
        },
    });

    function renderList($list: HTMLElement) {
        const jobs = store.jobs.get();
        if (!jobs.length) return;
        const pending = store.pending.get();
        ultraReplaceChildren($list, ...jobs.map((job) => JobRow(job, pending.get(job.jobId))));
    }

    function onListClick(event: Event) {
        const $target = event.target as HTMLElement;
        const externalUrl =
            $target.closest<HTMLAnchorElement>('[data-external-url]')?.dataset.externalUrl;
        if (externalUrl && isAndroidPlatform()) {
            event.preventDefault();
            void Browser.open({ url: externalUrl });
            return;
        }
        const retryJobId =
            $target.closest<HTMLButtonElement>('[data-retry-job-id]')?.dataset.retryJobId;
        if (retryJobId) {
            store.retry(retryJobId);
            return;
        }
        const deleteJobId =
            $target.closest<HTMLButtonElement>('[data-delete-job-id]')?.dataset.deleteJobId;
        if (deleteJobId) {
            store.remove(deleteJobId);
            return;
        }
        const stopJobId =
            $target.closest<HTMLButtonElement>('[data-stop-job-id]')?.dataset.stopJobId;
        const job = store.jobs.get().find((item) => item.jobId === stopJobId);
        if (job) store.stop(job);
    }

    function renderEmpty($p: HTMLElement) {
        if (store.error.get()) {
            $p.textContent = store.error.get();
            return;
        }
        $p.textContent = store.loaded.get() ? 'No active downloads.' : 'Loading downloads…';
    }

    return UltraComponent({
        component: '<section></section>',

        className: [styles.page],

        onMount: [
            () => DOCUMENT_TITLE_CONTEXT.setTitle('Downloads'),
            () => {
                store.load();
                const timer = setInterval(() => store.load(), POLL_INTERVAL_MS);
                return () => clearInterval(timer);
            },
        ],

        children: [
            UltraComponent({
                component: '<header></header>',
                className: [styles.header],
                children: [
                    `<span class="${styles.eyebrow}">Store</span>`,
                    `<h1 class="${styles.title}">Downloads</h1>`,
                ],
            }),

            UltraComponent({
                component: '<p></p>',
                className: [styles.empty],
                onMount: [renderEmpty],
                trigger: [
                    {
                        subscriber: [
                            store.jobs.subscribe,
                            store.error.subscribe,
                            store.loaded.subscribe,
                        ],
                        triggerFunction: ($p: HTMLElement) => {
                            $p.hidden = store.jobs.get().length > 0;
                            renderEmpty($p);
                        },
                    },
                ],
            }),

            UltraComponent({
                component: '<ul></ul>',
                className: [styles.list],
                eventHandler: { click: onListClick },
                onMount: [renderList],
                trigger: [
                    {
                        subscriber: [
                            store.jobs.subscribe,
                            store.pending.subscribe,
                        ],
                        triggerFunction: renderList,
                        defer: true,
                    },
                ],
            }),
        ],
    });
}
