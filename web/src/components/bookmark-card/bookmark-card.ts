import { UltraComponent, ultraState } from 'ultra-light-js';
import cardStyles from '@/components/comic-card/comic-card.module.css';
import readStyles from '@/components/comic-card/mark-as-read-button.module.css';
import moveStyles from '@/components/comic-card/move-button.module.css';
import downloadStyles from '@/components/comic-card/download-button.module.css';
import deleteStyles from '@/components/comic-card/delete-button.module.css';
import { ImageGen } from '@/components/image-generic/image-generic';
import { ReadBar } from '@/components/read-bar/read-bar';
import { StarComponent } from '@/components/star-component/star-component';
import { BOOKMARKS_CTX } from '@/context/bookmarks.context';
import { COMICS_TYPE_CTX } from '@/context/comics-types.context';
import { NO_IMAGE_URL } from '@/data';
import { BookmarkIcon } from '@/icons/bookmark.icon';
import { DownloadIcon } from '@/icons/download.icon';
import { FolderIcon } from '@/icons/folder.icon';
import { TrashIcon } from '@/icons/trash.icon';
import type { IStoreBookmark } from '@/store.types';

const noSubscription = () => () => {};

function ActionButton({
    className,
    label,
    icon,
    onClick,
}: {
    className: string[];
    label: string;
    icon: string;
    onClick: () => void;
}) {
    return UltraComponent({
        component: '<span></span>',
        className,
        attributes: { type: 'button', 'aria-label': label, title: label },
        eventHandler: {
            click: (e: Event) => {
                e.stopPropagation();
                e.preventDefault();
                onClick();
            },
        },
        children: [icon],
    });
}

function BookmarkRating({ bookmark }: { bookmark: IStoreBookmark }) {
    let $container: HTMLElement | null = null;

    const applyFill = (value: number) => {
        const stars = $container?.children;
        if (!stars) return;
        for (let i = 0; i < stars.length; i++) {
            const fraction = Math.max(0, Math.min(1, value - i));
            stars[i].querySelector('.fillRect')?.setAttribute('width', String(16 * fraction));
        }
    };

    return UltraComponent({
        component: '<div></div>',
        className: [cardStyles.rating],
        eventHandler: { mouseleave: () => applyFill(bookmark.rating) },
        onMount: [
            ($div: HTMLElement) => {
                $container = $div;
                $div.replaceChildren(
                    ...[0, 1, 2, 3, 4].map((i) =>
                        StarComponent({
                            weight: i + 1,
                            initialFill: Math.max(0, Math.min(1, bookmark.rating - i)),
                            onHover: applyFill,
                            changeRating: (rating) => BOOKMARKS_CTX.setRating(bookmark.key, rating),
                        }),
                    ),
                );
            },
        ],
    });
}

export function BookmarkCard({ bookmark }: { bookmark: IStoreBookmark }) {
    const [loaded, setLoaded, subsLoaded] = ultraState(false);

    const onCoverError = (e: Event) => {
        const $img = e.currentTarget as HTMLImageElement;
        if ($img.src === NO_IMAGE_URL) {
            setLoaded(true);
            return;
        }
        $img.src = NO_IMAGE_URL;
    };

    const onTypeChange = ($article: HTMLElement) => {
        $article.classList.toggle(cardStyles.detailMode, COMICS_TYPE_CTX.type.get() === 'detail');
    };

    const onBusyChange = ($button: HTMLElement) => {
        $button.setAttribute(
            'aria-busy',
            String(BOOKMARKS_CTX.downloading.get().includes(bookmark.key)),
        );
    };

    return UltraComponent({
        component: '<article></article>',
        className: [
            cardStyles.comicCard,
            ...(COMICS_TYPE_CTX.type.get() === 'detail' ? [cardStyles.detailMode] : []),
            ...(bookmark.read ? [cardStyles.isRead] : []),
        ],
        trigger: [
            { subscriber: COMICS_TYPE_CTX.type.subscribe, triggerFunction: onTypeChange },
        ],
        children: [
            UltraComponent({
                component: '<div></div>',
                className: [cardStyles.cover],
                children: [
                    UltraComponent({
                        component: '<a></a>',
                        trigger: [
                            {
                                subscriber: subsLoaded,
                                triggerFunction: ($a: HTMLElement) => {
                                    $a.classList.toggle(cardStyles.loaded, loaded());
                                },
                            },
                        ],
                        children: [
                            ImageGen({
                                attributes: {
                                    src: bookmark.thumbnailUrl || NO_IMAGE_URL,
                                    alt: bookmark.title,
                                    title: bookmark.title,
                                },
                                eventHandler: { load: () => setLoaded(true), error: onCoverError },
                            }),
                        ],
                    }),
                    ReadBar({
                        getReadPercentage: () => (bookmark.read ? 100 : 0),
                        subsReadPercentage: noSubscription,
                    }),
                ],
            }),
            UltraComponent({
                component: '<div></div>',
                className: [cardStyles.details],
                children: [
                    `<p class="${cardStyles.title}" title="${bookmark.title}">${bookmark.title}</p>`,
                    UltraComponent({
                        component: '<div></div>',
                        className: [cardStyles.actionsBlock],
                        children: [
                            BookmarkRating({ bookmark }),
                            UltraComponent({
                                component: '<div></div>',
                                className: [cardStyles.actions],
                                children: [
                                    UltraComponent({
                                        component: '<span></span>',
                                        className: [
                                            readStyles.markAsReadButton,
                                            ...(bookmark.read ? [readStyles.isRead] : []),
                                        ],
                                        attributes: {
                                            type: 'button',
                                            'aria-pressed': String(bookmark.read),
                                            'aria-label': bookmark.read
                                                ? 'Mark as unread'
                                                : 'Mark as read',
                                        },
                                        eventHandler: {
                                            click: () =>
                                                BOOKMARKS_CTX.setRead(bookmark.key, !bookmark.read),
                                        },
                                        children: [BookmarkIcon({ size: 16 })],
                                    }),
                                    UltraComponent({
                                        component: ActionButton({
                                            className: [downloadStyles.downloadComicButton],
                                            label: 'Download this post and remove the bookmark',
                                            icon: DownloadIcon({ size: 16 }),
                                            onClick: () => BOOKMARKS_CTX.download(bookmark.key),
                                        }),
                                        onMount: [onBusyChange],
                                        trigger: [
                                            {
                                                subscriber: BOOKMARKS_CTX.downloading.subscribe,
                                                triggerFunction: onBusyChange,
                                            },
                                        ],
                                    }),
                                    ActionButton({
                                        className: [moveStyles.moveButton],
                                        label: 'Move to another folder',
                                        icon: FolderIcon({ size: 18 }),
                                        onClick: () => BOOKMARKS_CTX.move(bookmark.key),
                                    }),
                                    ActionButton({
                                        className: [deleteStyles.deleteButton],
                                        label: 'Remove bookmark',
                                        icon: TrashIcon({ size: 20 }),
                                        onClick: () => BOOKMARKS_CTX.remove(bookmark.key),
                                    }),
                                ],
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}
