import type { ILibraryGroup, ILibraryResponseItem } from '../library.types';
import type { IStoreBookmark, IStorePost } from '../store.types';

export type TGridCell =
    | { type: 'item'; item: ILibraryResponseItem }
    | { type: 'bookmark'; bookmark: IStoreBookmark };

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
const spaced = (s: string) => s.replace(/_/g, ' ');

export function postKey(post: Pick<IStorePost, 'id' | 'link'>): string {
    return post.id ? `id:${post.id}` : `link:${post.link}`;
}

export function bookmarksInFolder(
    bookmarks: IStoreBookmark[],
    folder: string | undefined,
): IStoreBookmark[] {
    return bookmarks.filter((b) => (b.folder ?? undefined) === folder);
}

export function mergeBookmarks(
    items: ILibraryResponseItem[],
    bookmarks: IStoreBookmark[],
    sortByReleaseDate: boolean,
): TGridCell[] {
    const pending = sortByReleaseDate
        ? [...bookmarks]
        : [...bookmarks].sort((a, b) => collator.compare(spaced(a.title), spaced(b.title)));
    const out: TGridCell[] = [];
    for (const item of items) {
        while (
            !sortByReleaseDate &&
            pending.length &&
            !item.did &&
            collator.compare(spaced(pending[0].title), spaced(item.name)) < 0
        ) {
            out.push({ type: 'bookmark', bookmark: pending.shift()! });
        }
        out.push({ type: 'item', item });
    }
    for (const bookmark of pending) out.push({ type: 'bookmark', bookmark });
    return out;
}

export function cellKey(cell: TGridCell): string {
    if (cell.type === 'item') return cell.item.uid;
    const b = cell.bookmark;
    return `bookmark:${b.key}:${b.rating}:${b.read}`;
}

export function folderDir(groups: ILibraryGroup[], folder: string | undefined): string | undefined {
    if (!folder) return undefined;
    const group = groups.find((g) => g.uid === folder);
    const path =
        group?.path ?? groups.flatMap((g) => g.entries).find((e) => e.did && e.uid === folder)?.path;
    return path || undefined;
}

export function comicUids(groups: ILibraryGroup[]): Set<string> {
    return new Set(
        groups
            .flatMap((g) => g.entries)
            .filter((e) => !e.did)
            .map((e) => e.uid),
    );
}

export function onlyNewComic(groups: ILibraryGroup[], before: Set<string>): string | undefined {
    const fresh = groups.flatMap((g) => g.entries).filter((e) => !e.did && !before.has(e.uid));
    return fresh.length === 1 ? fresh[0].uid : undefined;
}
