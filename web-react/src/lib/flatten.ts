import type { ILibraryGroup } from '@/library.types';

export function flattenCounts(groups: ILibraryGroup[], uid?: string) {
    const start = uid && !groups.some((g) => g.uid === uid) ? uid : '';
    const all = groups.flatMap((g) => g.entries);
    let comics = 0;
    let folders = 0;
    const stack = [{ parent: start, direct: true }];
    for (let next = stack.pop(); next; next = stack.pop()) {
        for (const e of all) {
            if (e.parentId !== next.parent) continue;
            if (e.did) {
                folders++;
                stack.push({ parent: e.uid, direct: false });
            } else if (!next.direct) {
                comics++;
            }
        }
    }
    return { comics, folders };
}
