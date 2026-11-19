import styles from './mark-as-read-button.module.css';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { BookmarkIcon } from '@/icons/bookmark.icon';
import { toggleComicRead } from './mark-as-read-button.utils';

export function MarkAsReadButton({ uid }: { uid: string }) {
    const isRead = useComicCacheStore((s) => s.cache[uid]?.read === true);

    const onClick = () => toggleComicRead(uid);
    const label = isRead ? 'Mark this comic as unread' : 'Mark this comic as read';

    return (
        <button
            type="button"
            className={[styles.markAsReadButton, isRead ? styles.isRead : '']
                .filter(Boolean)
                .join(' ')}
            aria-pressed={isRead}
            aria-label={label}
            title={label}
            onClick={onClick}
        >
            <BookmarkIcon size={16} />
        </button>
    );
}
