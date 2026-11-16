import { useEffect, useMemo, useState } from 'react';
import { FolderCard } from '@/components/folder-card/folder-card';
import { ComicCard } from '@/components/comic-card/comic-card';
import { BookmarkCard } from '@/components/bookmark-card/bookmark-card';
import type { IComicFilters, ILibraryResponseItem } from '@/library.types';
import type { IStoreBookmark } from '@/store.types';
import { cellKey, mergeBookmarks } from '@/lib/bookmarks';

const PAGE_SIZE = 60;

export function ItemsGrid({
    items,
    filters,
    bookmarks,
    sortByReleaseDate = false,
}: {
    items: ILibraryResponseItem[];
    filters?: IComicFilters;
    bookmarks?: IStoreBookmark[];
    sortByReleaseDate?: boolean;
}) {
    const cells = useMemo(
        () => mergeBookmarks(items, bookmarks ?? [], sortByReleaseDate),
        [items, bookmarks, sortByReleaseDate],
    );
    const [count, setCount] = useState(PAGE_SIZE);

    const [sentinel, setSentinel] = useState<HTMLDivElement | null>(null);
    const hasMore = count < cells.length;

    useEffect(() => {
        if (!sentinel) return;
        const observer = new IntersectionObserver(
            (entries) => {
                if (entries.some((e) => e.isIntersecting)) setCount((c) => c + PAGE_SIZE);
            },
            { rootMargin: '600px' },
        );
        observer.observe(sentinel);
        return () => observer.disconnect();
    }, [sentinel]);

    return (
        <>
            {cells.slice(0, count).map((cell) => {
                const key = cellKey(cell);
                if (cell.type === 'bookmark')
                    return <BookmarkCard key={key} bookmark={cell.bookmark} />;
                const item = cell.item;
                return item.did ? (
                    <FolderCard key={key} title={item.name} uid={item.uid} />
                ) : (
                    <ComicCard key={key} item={item} filters={filters} />
                );
            })}
            {hasMore && (
                <div key={count} ref={setSentinel} style={{ gridColumn: '1 / -1', height: 1 }} />
            )}
        </>
    );
}
