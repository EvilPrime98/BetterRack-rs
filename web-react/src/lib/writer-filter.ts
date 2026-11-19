import type { ILibraryResponseItem } from '../library.types';

export const FILTERS_PATH = '/filters';

export function writerHref(writer: string): string {
    return `${FILTERS_PATH}?writer=${encodeURIComponent(writer)}`;
}

export function writerFromSearch(search: string): string {
    return (new URLSearchParams(search).get('writer') ?? '').trim();
}

export function itemWriters(item: ILibraryResponseItem): string[] {
    return item.comic?.credits?.writers ?? [];
}

export function filterByWriter(
    items: ILibraryResponseItem[],
    writer: string,
): ILibraryResponseItem[] {
    const comics = items.filter((item) => !item.did);
    if (!writer) return comics;
    return comics.filter((item) => itemWriters(item).includes(writer));
}
