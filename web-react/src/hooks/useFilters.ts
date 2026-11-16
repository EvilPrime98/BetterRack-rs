import { useState } from 'react';
import { FILTER_OPTIONS, type ILibraryFilters, type ILibraryResponseItem } from '../library.types';
import { useUserPrefStore } from '../stores/userPref.store';

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

export function applyFilters(
    rawItems: ILibraryResponseItem[],
    filters: ILibraryFilters,
): ILibraryResponseItem[] {
    const items = [...rawItems];
    if (filters.sortByReleaseDate) sortByReleaseDate(items);
    return items;
}

export function useFilters() {
    const [filters, setFiltersState] = useState<ILibraryFilters>(() => ({
        sortByCreation: false,
        sortByReleaseDate:
            useUserPrefStore.getState().getPref('filter') === FILTER_OPTIONS.byReleaseDate,
    }));

    function setFilters(updates: Partial<ILibraryFilters>) {
        setFiltersState((prev) => ({ ...prev, ...updates }));
    }

    function resetFilters() {
        setFiltersState({ sortByCreation: false, sortByReleaseDate: false });
    }

    return {
        filters,
        setFilters,
        resetFilters,
    };
}
