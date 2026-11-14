import type { IPullEntry } from '../store.types';

const normalize = (title: string) => title.replace(/_/g, ' ').toLowerCase();

export function matches(entry: IPullEntry, title: string): boolean {
    const keyword = normalize(entry.keyword.trim());
    return keyword !== '' && normalize(keywordFromTitle(title)) === keyword;
}

export function keywordFromTitle(title: string): string {
    const text = title.replace(/_/g, ' ');
    const end = text.search(/[#(]/);
    return (end === -1 ? text : text.slice(0, end)).replace(/[\d\s-]+$/, '').trim();
}
