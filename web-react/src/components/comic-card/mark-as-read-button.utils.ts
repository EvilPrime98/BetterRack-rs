import { useComicCacheStore } from '@/stores/comicCache.store';

export function toggleComicRead(uid: string) {
    const { getCacheById, setCacheById } = useComicCacheStore.getState();
    const currCache = getCacheById(uid);
    const isRead = currCache?.read;
    const newRead = isRead === undefined ? true : !isRead;
    setCacheById(uid, {
        read: newRead,
        readPer: newRead ? 100 : 0,
    });
}
