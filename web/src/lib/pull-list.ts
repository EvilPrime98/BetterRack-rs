import type { IPullEntry } from '../store.types';

const normalize = (title: string) => title.replace(/_/g, ' ').toLowerCase();

export function matches(entry: IPullEntry, title: string): boolean {
    const keyword = entry.keyword.trim().toLowerCase();
    if (!keyword) return false;
    const text = normalize(title);
    if (!text.includes(keyword)) return false;
    return !entry.exclude
        .split(',')
        .map((t) => t.trim().toLowerCase())
        .filter(Boolean)
        .some((t) => text.includes(t));
}

export function keywordFromTitle(title: string): string {
    const text = title.replace(/_/g, ' ');
    const end = text.search(/[#(]/);
    return (end === -1 ? text : text.slice(0, end)).replace(/[\d\s-]+$/, '').trim();
}
