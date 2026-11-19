import { useEffect, useRef, useState, type SyntheticEvent } from 'react';
import styles from './store-card.module.css';
import { ImageGen } from '@/components/image-generic/image-generic';
import { NO_IMAGE_URL } from '@/data';
import { useDownloadDirModalContext } from '@/context/DownloadDirModalContext.hooks';
import { useLinkPickerModalContext } from '@/context/LinkPickerModalContext.hooks';
import { getComicLinks, startDownloadJob } from '@/services/store.service';
import { toast } from '@/services/toast.service';
import { STRAT, type IStoreLink, type IStorePost, type TCardState } from '@/store.types';
import { BRButton } from '@/components/br-button/br-button';
import { DOWNLOAD_TRACKER } from '@/services/download-guard.service';
import { BookmarkIcon } from '@/icons/bookmark.icon';
import { useBookmarksStore } from '@/stores/bookmarks.store';
import { usePullListStore } from '@/stores/pullList.store';
import { useDownloadedStore } from '@/stores/downloaded.store';
import { useBookmarkActions } from '@/hooks/useBookmarkActions';
import { postKey } from '@/lib/bookmarks';
import { matches } from '@/lib/pull-list';
import { ComicCard } from '@/components/comic-card/comic-card';
import { useLibraryStore } from '@/stores/library.store';
import { findEntryByPath } from '@/lib/downloaded';

export function StoreCard({ item }: { item: IStorePost }) {
    const path = useDownloadedStore((s) => (item.id != null ? s.paths.get(item.id) : undefined));
    const entry = useLibraryStore((s) => (path ? findEntryByPath(s.groups, path) : undefined));

    if (entry) return <ComicCard item={entry} forceCover />;
    return <PlainStoreCard item={item} />;
}

function PlainStoreCard({ item }: { item: IStorePost }) {
    const { openDownloadDirModal } = useDownloadDirModalContext();
    const { openLinkPickerModal } = useLinkPickerModalContext();

    const { toggle: toggleBookmark } = useBookmarkActions();
    const bookmarked = useBookmarksStore((s) => s.items.some((b) => b.key === postKey(item)));
    const downloaded = useDownloadedStore((s) => item.id != null && s.ids.has(item.id));
    const followed = usePullListStore((s) => s.items.some((e) => matches(e, item.title)));

    const [state, setState] = useState<TCardState>({ status: 'idle' });
    const [coverLoaded, setCoverLoaded] = useState(false);
    const lastRenderable = useRef<TCardState>(state);
    const displayState = state.status === 'links-loading' ? lastRenderable.current : state;

    async function startDownload(link: IStoreLink) {
        if (!item.id) return;

        const outputDir = await openDownloadDirModal(link.title);

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

            const chosen = await openLinkPickerModal(links, item.title);

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

    function onCoverLoad() {
        setCoverLoaded(true);
    }

    function onCoverError(e: SyntheticEvent<HTMLImageElement>) {
        const $img = e.currentTarget;
        if ($img.src === NO_IMAGE_URL) {
            setCoverLoaded(true);
            return;
        }
        $img.src = NO_IMAGE_URL;
    }

    function renderAction(curr: TCardState) {
        if (curr.status === 'idle') {
            return (
                <BRButton text="" onClick={onDownloadClick} variant="classic">
                    <span>{downloaded ? '✓ Downloaded' : 'Download'}</span>
                </BRButton>
            );
        }

        if (curr.status === 'error') {
            return (
                <BRButton
                    text=""
                    variant="classic"
                    className={styles.retryBtn}
                    onClick={onDownloadClick}
                >
                    <span>Retry</span>
                </BRButton>
            );
        }

        return null;
    }

    useEffect(() => {
        if (state.status !== 'links-loading') {
            lastRenderable.current = state;
        }
    }, [state]);

    return (
        <article className={styles.storeCard}>
            <div
                className={[styles.cover, coverLoaded ? styles.loaded : '']
                    .filter(Boolean)
                    .join(' ')}
            >
                <ImageGen
                    src={item.thumbnailUrl || NO_IMAGE_URL}
                    alt={item.title}
                    title={item.title}
                    onLoad={onCoverLoad}
                    onError={onCoverError}
                />

                {downloaded && (
                    <span
                        className={styles.downloadedBadge}
                        title="Downloaded"
                        aria-label="Downloaded"
                    >
                        ✓
                    </span>
                )}

                <button
                    type="button"
                    className={[styles.bookmarkBtn, bookmarked ? styles.marked : '']
                        .filter(Boolean)
                        .join(' ')}
                    aria-pressed={bookmarked}
                    aria-label={bookmarked ? 'Remove bookmark' : 'Bookmark this post'}
                    onClick={() => toggleBookmark(item)}
                >
                    <BookmarkIcon size={18} />
                </button>

                <button
                    type="button"
                    className={[styles.pullBtn, followed ? styles.marked : '']
                        .filter(Boolean)
                        .join(' ')}
                    aria-pressed={followed}
                    aria-label={followed ? 'Remove from pull list' : 'Add to pull list'}
                    onClick={() => usePullListStore.getState().toggle(item.title)}
                >
                    {followed ? '✓ Pull list' : '+ Pull list'}
                </button>
            </div>

            <div className={styles.details}>
                <a href={item.link} target="_blank" rel="noopener noreferrer">
                    <p className={styles.title} title={item.title}>
                        {item.title}
                    </p>
                </a>

                {item.uploadDate && (
                    <p className={styles.date}>{new Date(item.uploadDate).toLocaleDateString()}</p>
                )}

                <div className={styles.action}>{renderAction(displayState)}</div>
            </div>
        </article>
    );
}
