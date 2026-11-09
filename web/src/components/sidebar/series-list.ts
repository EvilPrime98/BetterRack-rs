import type { ILibraryGroup } from '../../library.types';
import { SideBarElement } from './sider-bar-element';

const SERIES_PAGE_SIZE = 50;

export function SeriesList({
    groups,
    isHidden,
}: {
    groups: () => ILibraryGroup[];
    isHidden: () => boolean;
}) {
    let count = SERIES_PAGE_SIZE;
    let observer: IntersectionObserver | null = null;
    const $sentinel = document.createElement('div');
    $sentinel.style.height = '1px';

    function append($nav: HTMLElement, from: number, to: number) {
        const all = groups();
        $sentinel.remove();
        $nav.append(
            ...all.slice(from, to).map((group) =>
                SideBarElement({
                    item: {
                        uid: group.uid,
                        did: true,
                        name: group.name,
                        path: group.path,
                        parentId: '',
                        createdAt: 0,
                    },
                }),
            ),
        );
        if (to < all.length) $nav.appendChild($sentinel);
    }

    function disconnect() {
        observer?.disconnect();
        observer = null;
    }

    function render($nav: HTMLElement) {
        disconnect();
        count = SERIES_PAGE_SIZE;
        $nav.replaceChildren();
        if (isHidden()) return;

        append($nav, 0, count);

        observer = new IntersectionObserver(
            (entries) => {
                if (!entries.some((e) => e.isIntersecting)) return;
                const from = count;
                count += SERIES_PAGE_SIZE;
                observer?.unobserve($sentinel);
                append($nav, from, count);
                if ($sentinel.isConnected) observer?.observe($sentinel);
            },
            { rootMargin: '300px' },
        );
        observer.observe($sentinel);
    }

    return { render, disconnect };
}
