import { UltraComponent, ultraState } from 'ultra-light-js';
import styles from './store-card.module.css';
import { ImageGen } from '@/components/image-generic/image-generic';
import { NO_IMAGE_URL } from '@/data';
import { DOWNLOAD_DIR_MODAL_CTX } from '@/context/download-dir-modal.context';
import { LINK_PICKER_MODAL_CTX } from '@/context/link-picker-modal.context';
import { DOWNLOAD_TRACKER } from '@/services/download-guard.service';
import { getComicLinks, startDownloadJob } from '@/services/store.service';
import { toast } from '@/services/toast.service';
import { STRAT, type IStoreLink, type IStorePost, type TCardState } from '@/store.types';
import { BRButton } from '@/components/br-button/br-button';
import { BookmarkIcon } from '@/icons/bookmark.icon';
import { BOOKMARKS_CTX } from '@/context/bookmarks.context';
import { postKey } from '@/lib/bookmarks';
import { PULL_LIST_CTX } from '@/context/pull-list.context';

export function StoreCard({ item }: { item: IStorePost }) {
    const [state, setState, subsState] = ultraState<TCardState>({
        status: 'idle',
    });

    const [coverLoaded, setCoverLoaded, subsCoverLoaded] = ultraState(false);

    async function startDownload(link: IStoreLink) {
        if (!item.id) return;

        const outputDir = await DOWNLOAD_DIR_MODAL_CTX.openDownloadDirModal(link.title);

        if (!outputDir) {
            setState({ status: 'idle' });
            return;
        }

        try {
            await startDownloadJob({
                id: item.id,
                title: link.title,
                uuid: link.uuid,
                outputDir,
                strat: STRAT,
            });
            DOWNLOAD_TRACKER.started();
            setState({ status: 'idle' });
            toast.success('Download in progress');
        } catch (e) {
            const message = e instanceof Error ? e.message : 'Download failed.';
            setState({ status: 'error', message });
            toast.error(message);
        }
    }

    async function onDownloadClick() {
        if (!item.id) return;

        setState({ status: 'links-loading' });

        try {
            const links = await getComicLinks(item.id, STRAT);
            if (links.length === 0) {
                setState({ status: 'error', message: 'No download links found.' });
                return;
            }
            if (links.length === 1) {
                await startDownload(links[0]);
                return;
            }

            const chosen = await LINK_PICKER_MODAL_CTX.openLinkPickerModal(links, item.title);

            if (!chosen) {
                setState({ status: 'idle' });
                return;
            }

            await startDownload(chosen);
        } catch (e) {
            const message = e instanceof Error ? e.message : 'Failed to fetch links.';
            setState({ status: 'error', message });
            toast.error(message);
        }
    }

    function onBookmarkChange($button: HTMLElement) {
        const marked = BOOKMARKS_CTX.contains(postKey(item));
        $button.classList.toggle(styles.marked, marked);
        $button.setAttribute('aria-pressed', String(marked));
        $button.setAttribute('aria-label', marked ? 'Remove bookmark' : 'Bookmark this post');
    }

    function onPullChange($button: HTMLElement) {
        const followed = PULL_LIST_CTX.follows(item.title);
        $button.classList.toggle(styles.marked, followed);
        $button.setAttribute('aria-pressed', String(followed));
        $button.textContent = followed ? '✓ Pull list' : '+ Pull list';
        $button.setAttribute('aria-label', followed ? 'Remove from pull list' : 'Add to pull list');
    }

    function onCoverLoad() {
        setCoverLoaded(true);
    }

    function onCoverError(e: Event) {
        const $img = e.currentTarget as HTMLImageElement;
        if ($img.src === NO_IMAGE_URL) {
            setCoverLoaded(true);
            return;
        }
        $img.src = NO_IMAGE_URL;
    }

    function onActionChange($div: HTMLElement) {
        const curr = state();

        const $button = (
            label: string,
            {
                disabled = false,
                loading = false,
                retry = false,
            }: {
                disabled?: boolean;
                loading?: boolean;
                retry?: boolean;
            } = {},
        ) => {
            return BRButton({
                text: '',
                variant: 'classic',
                className: retry ? [styles.retryBtn] : [],
                attributes: disabled ? { type: 'button', disabled: 'true' } : { type: 'button' },
                eventHandler: { click: onDownloadClick },
                children: [
                    loading ? `<span class="${styles.spinner}"></span>` : '',
                    `<span>${label}</span>`,
                ],
            });
        };

        if (curr.status === 'idle') {
            $div.replaceChildren($button('Download'));
        } else if (curr.status === 'error') {
            $div.replaceChildren($button('Retry', { retry: true }));
        }
    }

    return UltraComponent({
        component: '<article></article>',

        className: [styles.storeCard],

        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.cover],
                trigger: [
                    {
                        subscriber: subsCoverLoaded,
                        triggerFunction: ($div: HTMLElement) => {
                            $div.classList.toggle(styles.loaded, coverLoaded());
                        },
                    },
                ],
                children: [
                    ImageGen({
                        attributes: {
                            src: item.thumbnailUrl || NO_IMAGE_URL,
                            alt: item.title,
                            title: item.title,
                        },
                        eventHandler: { load: onCoverLoad, error: onCoverError },
                    }),
                    UltraComponent({
                        component: '<button></button>',
                        className: [styles.bookmarkBtn],
                        attributes: { type: 'button' },
                        eventHandler: { click: () => BOOKMARKS_CTX.toggle(item) },
                        onMount: [onBookmarkChange],
                        trigger: [
                            {
                                subscriber: BOOKMARKS_CTX.items.subscribe,
                                triggerFunction: onBookmarkChange,
                            },
                        ],
                        children: [BookmarkIcon({ size: 18 })],
                    }),
                    UltraComponent({
                        component: '<button></button>',
                        className: [styles.pullBtn],
                        attributes: { type: 'button' },
                        eventHandler: { click: () => PULL_LIST_CTX.toggle(item.title) },
                        onMount: [onPullChange],
                        trigger: [
                            {
                                subscriber: PULL_LIST_CTX.items.subscribe,
                                triggerFunction: onPullChange,
                            },
                        ],
                    }),
                ],
            }),

            UltraComponent({
                component: '<div></div>',
                className: [styles.details],
                children: [
                    `<p class="${styles.title}" title="${item.title}">${item.title}</p>`,

                    ...(item.uploadDate
                        ? [
                              `<p class="${styles.date}">${new Date(item.uploadDate).toLocaleDateString()}</p>`,
                          ]
                        : []),

                    UltraComponent({
                        component: '<div></div>',
                        className: [styles.action],
                        onMount: [onActionChange],
                        trigger: [
                            {
                                subscriber: subsState,
                                triggerFunction: onActionChange,
                            },
                        ],
                    }),
                ],
            }),
        ],
    });
}
