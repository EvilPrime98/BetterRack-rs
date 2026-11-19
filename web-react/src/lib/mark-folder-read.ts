import { useComicCacheStore } from '@/stores/comicCache.store';

export function markComicsRead(uids: string[]) {
    const { setCacheById } = useComicCacheStore.getState();
    for (const uid of uids) setCacheById(uid, { read: true, readPer: 100 });
}
