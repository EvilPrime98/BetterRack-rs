import type { ILibraryResponseItem, TFilterOptions } from '../library.types';
import { useUserPrefStore } from '../stores/userPref.store';

function releaseKey(item: ILibraryResponseItem): number | null {
    const d = item.comic?.releaseDate;
    if (!d) return null;
    const year = Number(d.releaseYear);
    if (!year) return null;
    return year * 10000 + (Number(d.releaseMonth) || 0) * 100 + (Number(d.releaseDay) || 0);
}

export function applySort(
    rawItems: ILibraryResponseItem[],
    sort: TFilterOptions,
): ILibraryResponseItem[] {
    const items = [...rawItems];
    if (sort === 'Creation Date') items.sort((a, b) => b.createdAt - a.createdAt);
    if (sort === 'Release Date')
        items.sort((a, b) => {
            const ka = releaseKey(a);
            const kb = releaseKey(b);
            if (ka === null || kb === null) return Number(ka === null) - Number(kb === null);
            return ka - kb;
        });
    return items;
}

export function useSort() {
    const sort = useUserPrefStore((s) => s.pref.filter);
    const setSort = (filter: TFilterOptions) => useUserPrefStore.getState().setPref({ filter });
    return { sort, setSort };
}
