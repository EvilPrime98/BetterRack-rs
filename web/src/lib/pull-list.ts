import type { IPullEntry, IStorePost } from '../store.types';

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

export function isNew(entry: IPullEntry, post: IStorePost): boolean {
    return matches(entry, post.title) && post.id !== undefined && post.id > entry.lastSeenId;
}

export function newCount(entry: IPullEntry, posts: IStorePost[]): number {
    return posts.filter((p) => isNew(entry, p)).length;
}

export function latestMatchId(entry: IPullEntry, posts: IStorePost[]): number {
    return posts
        .filter((p) => matches(entry, p.title))
        .reduce((max, p) => Math.max(max, p.id ?? 0), entry.lastSeenId);
}

export function keywordFromTitle(title: string): string {
    const text = title.replace(/_/g, ' ');
    const end = text.search(/[#(]/);
    return (end === -1 ? text : text.slice(0, end)).replace(/[\d\s-]+$/, '').trim();
}
