import type { ILibraryGroup, ILibraryResponseItem } from '@/library.types';

function pathKey(path: string): string {
    return path.replaceAll('\\', '/').replace(/\/+$/, '').toLowerCase();
}

export function findEntryByPath(
    groups: ILibraryGroup[],
    path: string,
): ILibraryResponseItem | undefined {
    const key = pathKey(path);
    for (const group of groups) {
        const entry = group.entries.find((e) => !e.did && pathKey(e.path) === key);
        if (entry) return entry;
    }
    return undefined;
}
