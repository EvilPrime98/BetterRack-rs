import { UltraComponent } from 'ultra-light-js';
import styles from './folder-card.module.css';
import { FolderStackCard } from './folder-stack-card';
import type { ILibraryResponseItem } from '@/library.types';
import { COMIC_CACHE_CONTEXT } from '@/context/comic-cache.context';
import { API_URL } from '@/services/library.service';
import { withAuthQuery } from '@/services/server-config.service';

export function FolderCardStack({
    stackCovers,
}: {
    title: string;
    stackCovers: ILibraryResponseItem[];
}) {
    return UltraComponent({
        component: '<span></span>',
        className: [styles.cover, styles.stack],
        children: stackCovers.map((item) => {
            const comicCache = COMIC_CACHE_CONTEXT.getCacheById(item.uid);
            return FolderStackCard({
                cover: withAuthQuery(`${API_URL}/api/thumbnail/${item.uid}`),
                isRead: comicCache?.read || false,
            });
        }),
    });
}
