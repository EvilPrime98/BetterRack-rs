import { UltraComponent } from 'ultra-light-js';
import styles from './mark-as-read-button.module.css';
import { COMIC_CACHE_CONTEXT } from '@/context/comic-cache.context';
import { BookmarkIcon } from '@/icons/bookmark.icon';

export function toggleComicRead(uid: string) {
    const currCache = COMIC_CACHE_CONTEXT.getCacheById(uid);
    const isRead = currCache?.read;
    const newRead = isRead === undefined ? true : !isRead;
    COMIC_CACHE_CONTEXT.setCacheById(uid, {
        read: newRead,
        readPer: newRead ? 100 : 0,
    });
}

export function MarkAsReadButton({ uid }: { uid: string }) {
    const getCache = () => COMIC_CACHE_CONTEXT.getCacheById(uid);

    const getIsRead = () => getCache()?.read === true;

    const onButtonStateChange = ($button: HTMLElement) => {
        const isRead = getIsRead();
        $button.classList.toggle(styles.isRead, isRead);
        $button.setAttribute('aria-pressed', String(isRead));
        $button.setAttribute(
            'aria-label',
            isRead ? 'Mark this comic as unread' : 'Mark this comic as read',
        );
    };

    const onClick = () => toggleComicRead(uid);

    return UltraComponent({
        component: '<span></span>',
        className: [styles.markAsReadButton, ...(getIsRead() ? [styles.isRead] : [])],
        attributes: {
            type: 'button',
            'aria-pressed': String(getIsRead()),
            'aria-label': getIsRead() ? 'Mark this comic as unread' : 'Mark this comic as read',
        },
        eventHandler: {
            click: onClick,
        },
        trigger: [
            {
                subscriber: COMIC_CACHE_CONTEXT.cache.subscribe,
                triggerFunction: onButtonStateChange,
            },
        ],
        children: [BookmarkIcon({ size: 16 })],
    });
}
