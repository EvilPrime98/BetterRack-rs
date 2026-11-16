import type { ILibraryGroup } from '@/library.types';
import { useEffect, useRef, useState } from 'react';
import { SideBarElement } from './sider-bar-element';

const SERIES_PAGE_SIZE = 50;

export function SeriesList({ groups, isHidden }: { groups: ILibraryGroup[]; isHidden: boolean }) {
    const [count, setCount] = useState(SERIES_PAGE_SIZE);

    const sentinelRef = useRef<HTMLDivElement>(null);
    const hasMore = !isHidden && count < groups.length;

    useEffect(() => {
        const node = sentinelRef.current;
        if (!hasMore || !node) return;
        const observer = new IntersectionObserver(
            (entries) => {
                if (entries.some((e) => e.isIntersecting)) setCount((c) => c + SERIES_PAGE_SIZE);
            },
            { rootMargin: '300px' },
        );
        observer.observe(node);
        return () => observer.disconnect();
    }, [hasMore, count]);

    return (
        <>
            {!isHidden &&
                groups.slice(0, count).map((group) => (
                    <SideBarElement
                        key={group.uid}
                        item={{
                            uid: group.uid,
                            did: true,
                            name: group.name,
                            path: group.path,
                            parentId: '',
                            createdAt: 0,
                        }}
                    />
                ))}
            {hasMore && <div ref={sentinelRef} style={{ height: 1 }} />}
        </>
    );
}
