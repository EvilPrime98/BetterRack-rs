import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import {
    FILTER_OPTIONS,
    isIUltraCompStateStateful,
    type ILibraryFilters,
    type ILibraryResponseItem,
} from '../library.types';
import { USER_PREF } from '../context/user-pref-cache.context';

export function ultraFilters({
    rawItems,
    setItems,
}: {
    rawItems: () => ILibraryResponseItem[];
    setItems: (value: ILibraryResponseItem[]) => void;
}) {
    const filters: ILibraryFilters = ultraCompState({
        sortByCreation: false,
        sortByReleaseDate: false,
    });

    function sortByReleaseDate(data: ILibraryResponseItem[]): ILibraryResponseItem[] {
        return data.sort((a, b) => {
            const aDate = a.comic?.releaseDate;
            const bDate = b.comic?.releaseDate;
            if (!aDate || !bDate) return -1;
            const r1 = Number(aDate.releaseYear) - Number(bDate.releaseYear);
            if (r1 !== 0) return r1;
            const r2 = Number(aDate.releaseMonth) - Number(bDate.releaseMonth);
            if (r2 !== 0) return r2;
            return Number(aDate.releaseDay) - Number(bDate.releaseDay);
        });
    }

    function applyFilters() {
        const source = [...rawItems()];
        if (filters.sortByReleaseDate.get()) sortByReleaseDate(source);
        setItems(source);
    }

    function resetFilters() {
        for (const key of Object.keys(filters)) {
            const keyVal = filters[key as keyof ILibraryFilters];
            if (isIUltraCompStateStateful(keyVal)) {
                keyVal.set(false);
            }
        }
        setItems(rawItems());
    }

    Object.values(filters).forEach((f: IUltraCompStateStateful<boolean>) =>
        f.subscribe(applyFilters),
    );

    filters.sortByReleaseDate.set(USER_PREF.getPref('filter') === FILTER_OPTIONS.byReleaseDate);

    return {
        filters,
        resetFilters,
        applyFilters,
    };
}
