import { memo, useState, type MouseEvent, type SyntheticEvent } from 'react';
import cardStyles from '@/components/comic-card/comic-card.module.css';
import readStyles from '@/components/comic-card/mark-as-read-button.module.css';
import moveStyles from '@/components/comic-card/move-button.module.css';
import downloadStyles from '@/components/comic-card/download-button.module.css';
import deleteStyles from '@/components/comic-card/delete-button.module.css';
import { ImageGen } from '@/components/image-generic/image-generic';
import { ReadBar } from '@/components/read-bar/read-bar';
import { StarComponent } from '@/components/star-component/star-component';
import { NO_IMAGE_URL } from '@/data';
import { BookmarkIcon } from '@/icons/bookmark.icon';
import { DownloadIcon } from '@/icons/download.icon';
import { FolderIcon } from '@/icons/folder.icon';
import { TrashIcon } from '@/icons/trash.icon';
import { useBookmarkActions } from '@/hooks/useBookmarkActions';
import { useBookmarksStore } from '@/stores/bookmarks.store';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import type { IStoreBookmark } from '@/store.types';

function BookmarkRating({ bookmark }: { bookmark: IStoreBookmark }) {
    const [preview, setPreview] = useState<number | null>(null);
    const display = Math.max(0, Math.min(5, preview ?? bookmark.rating));

    return (
        <div className={cardStyles.rating} onMouseLeave={() => setPreview(null)}>
            {[0, 1, 2, 3, 4].map((i) => (
                <StarComponent
                    key={i}
                    weight={i + 1}
                    initialFill={Math.max(0, Math.min(1, display - i))}
                    onHover={setPreview}
                    changeRating={(rating) =>
                        useBookmarksStore
                            .getState()
                            .patch(bookmark.key, { rating }, 'Failed to rate bookmark.')
                    }
                />
            ))}
        </div>
    );
}

export const BookmarkCard = memo(function BookmarkCard({ bookmark }: { bookmark: IStoreBookmark }) {
    const { move, download } = useBookmarkActions();
    const comicsType = useComicsTypeStore((s) => s.type);
    const busy = useBookmarksStore((s) => s.downloading.includes(bookmark.key));
    const [loaded, setLoaded] = useState(false);

    const onCoverError = (e: SyntheticEvent<HTMLImageElement>) => {
        if (e.currentTarget.src === NO_IMAGE_URL) {
            setLoaded(true);
            return;
        }
        e.currentTarget.src = NO_IMAGE_URL;
    };

    const act = (run: () => void) => (e: MouseEvent) => {
        e.stopPropagation();
        e.preventDefault();
        run();
    };

    const readLabel = bookmark.read ? 'Mark as unread' : 'Mark as read';

    return (
        <article
            className={[
                cardStyles.comicCard,
                comicsType === 'detail' ? cardStyles.detailMode : '',
                bookmark.read ? cardStyles.isRead : '',
            ]
                .filter(Boolean)
                .join(' ')}
        >
            <div className={cardStyles.cover}>
                <div className={cardStyles.board} />

                <div className={[cardStyles.frame, loaded ? cardStyles.loaded : ''].join(' ')}>
                    <ImageGen
                        src={bookmark.thumbnailUrl || NO_IMAGE_URL}
                        alt={bookmark.title}
                        title={bookmark.title}
                        onLoad={() => setLoaded(true)}
                        onError={onCoverError}
                    />
                </div>

                <div className={cardStyles.bagOverlay} />

                <ReadBar readPercentage={bookmark.read ? 100 : 0} />
            </div>

            <div className={cardStyles.details}>
                <p className={cardStyles.title} title={bookmark.title}>
                    {bookmark.title}
                </p>

                <div className={cardStyles.actionsBlock}>
                    <BookmarkRating bookmark={bookmark} />

                    <div className={cardStyles.actions}>
                        <button
                            type="button"
                            className={[
                                readStyles.markAsReadButton,
                                bookmark.read ? readStyles.isRead : '',
                            ]
                                .filter(Boolean)
                                .join(' ')}
                            aria-pressed={bookmark.read}
                            aria-label={readLabel}
                            title={readLabel}
                            onClick={() =>
                                useBookmarksStore
                                    .getState()
                                    .patch(
                                        bookmark.key,
                                        { read: !bookmark.read },
                                        'Failed to update bookmark.',
                                    )
                            }
                        >
                            <BookmarkIcon size={16} />
                        </button>

                        <button
                            type="button"
                            className={downloadStyles.downloadComicButton}
                            aria-label="Download this post and remove the bookmark"
                            title="Download this post and remove the bookmark"
                            aria-busy={busy}
                            onClick={act(() => download(bookmark))}
                        >
                            <DownloadIcon size={16} />
                        </button>

                        <button
                            type="button"
                            className={moveStyles.moveButton}
                            aria-label="Move to another folder"
                            title="Move to another folder"
                            onClick={act(() => move(bookmark.key))}
                        >
                            <FolderIcon size={18} />
                        </button>

                        <button
                            type="button"
                            className={deleteStyles.deleteButton}
                            aria-label="Remove bookmark"
                            title="Remove bookmark"
                            onClick={act(() => useBookmarksStore.getState().remove(bookmark.key))}
                        >
                            <TrashIcon size={20} />
                        </button>
                    </div>
                </div>
            </div>
        </article>
    );
});
