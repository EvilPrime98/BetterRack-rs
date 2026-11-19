import styles from './folder-card.module.css';
import { FolderStackCard } from './folder-stack-card';
import type { ILibraryResponseItem } from '@/library.types';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { API_URL } from '@/services/library.service';
import { withAuthQuery } from '@/services/server-config.service';

export function FolderCardStack({
    stackCovers,
}: {
    title: string;
    stackCovers: ILibraryResponseItem[];
}) {
    const cache = useComicCacheStore((s) => s.cache);

    return (
        <span className={[styles.cover, styles.stack].join(' ')}>
            {stackCovers.map((item) => {
                const comicCache = cache[item.uid];
                return (
                    <FolderStackCard
                        key={item.uid}
                        cover={withAuthQuery(`${API_URL}/api/thumbnail/${item.uid}`)}
                        isRead={comicCache?.read || false}
                    />
                );
            })}
        </span>
    );
}
