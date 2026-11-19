import { useEffect, useMemo, useState } from 'react';
import styles from './store.page.module.css';
import { Layout } from '@/layout';
import { StoreCard } from '@/components/store-card/store-card';
import { getLatestComics } from '@/services/store.service';
import { toast } from '@/services/toast.service';
import type { IStorePost } from '@/store.types';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { usePullListStore } from '@/stores/pullList.store';
import { useDownloadedStore } from '@/stores/downloaded.store';
import { postKey } from '@/lib/bookmarks';
import { matches } from '@/lib/pull-list';
import { useSettingsStore } from '@/stores/settings.store';
import { dayStart, rangeLabel, today, weekStart } from '@/lib/week';

const PAGE_SIZE = 100;
const MAX_PAGES = 10;

export function PullListPage() {
    const setTitle = useDocumentTitleStore((s) => s.setTitle);
    const entries = usePullListStore((s) => s.items);
    const configured = useSettingsStore((s) => s.settings.apiUrl.trim() !== '');
    const [error, setError] = useState('');
    const currentWeek = useMemo(() => weekStart(today()), []);
    const [selectedWeek, setSelectedWeek] = useState(currentWeek);
    const [weekPosts, setWeekPosts] = useState<IStorePost[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        useDownloadedStore.getState().load().catch(() => undefined);
    }, []);

    useEffect(() => {
        setTitle('Pull list');
    }, [setTitle]);

    useEffect(() => {
        let cancelled = false;
        const after = dayStart(selectedWeek);
        const before = dayStart(selectedWeek + 7);
        setWeekPosts([]);
        setError('');
        if (!configured) {
            setLoading(false);
            return;
        }
        setLoading(true);
        (async () => {
            try {
                const seen = new Set<string>();
                const all: IStorePost[] = [];
                for (let page = 1; page <= MAX_PAGES; page++) {
                    const items = await getLatestComics({
                        page,
                        perPage: PAGE_SIZE,
                        after,
                        before,
                    });
                    if (cancelled) return;
                    for (const item of items) {
                        const key = postKey(item);
                        if (seen.has(key)) continue;
                        seen.add(key);
                        all.push(item);
                    }
                    if (items.length < PAGE_SIZE) break;
                }
                if (!cancelled) setWeekPosts(all);
            } catch (e) {
                if (cancelled) return;
                const message = e instanceof Error ? e.message : 'Failed to load comics.';
                setError(message);
                toast.error(message);
            }
            if (!cancelled) setLoading(false);
        })();
        return () => {
            cancelled = true;
        };
    }, [selectedWeek, configured]);

    const posts = useMemo(
        () => weekPosts.filter((p) => entries.some((e) => matches(e, p.title))),
        [weekPosts, entries],
    );

    const emptyText = !configured
        ? 'The store source is not configured. Set it in Settings.'
        : entries.length === 0
          ? 'Use the pin on a store card to follow a series.'
          : loading
            ? 'Loading…'
            : 'No posts match this week.';

    const atCurrent = selectedWeek >= currentWeek;

    return (
        <Layout>
            <section className={styles.page}>
                <h1 className={styles.title}>Pull list</h1>

                <div className={styles.weekNav}>
                    <button
                        type="button"
                        className={styles.weekBtn}
                        aria-label="Previous week"
                        onClick={() => setSelectedWeek(selectedWeek - 7)}
                    >
                        ‹
                    </button>
                    <span className={styles.weekLabel}>{rangeLabel(selectedWeek)}</span>
                    <button
                        type="button"
                        className={styles.weekBtn}
                        aria-label="Next week"
                        disabled={atCurrent}
                        onClick={() => setSelectedWeek(selectedWeek + 7)}
                    >
                        ›
                    </button>
                    <button
                        type="button"
                        className={styles.weekBtn}
                        aria-label="Jump to this week"
                        disabled={atCurrent}
                        onClick={() => setSelectedWeek(currentWeek)}
                    >
                        This week
                    </button>
                </div>

                <p className={styles.errorText}>{error}</p>

                <section className={styles.grid}>
                    {posts.length === 0 && <p className={styles.empty}>{emptyText}</p>}
                    {configured && posts.map((post) => (
                        <StoreCard key={postKey(post)} item={post} />
                    ))}
                </section>
            </section>
        </Layout>
    );
}
