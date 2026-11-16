import { useNavigate } from 'react-router-dom';
import { ItemsGrid } from '@/components/items-grid/items-grid';
import { Layout } from '@/layout';
import { useLibraryStore } from '@/stores/library.store';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import { useComicFilters } from '@/hooks/useComicFilters';
import { COMIC_FILTERS } from '@/library.types';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import { CloseIcon } from '@/icons/close.icon';
import styles from './filtered-page.module.css';

export function FilterPage() {
    const navigate = useNavigate();
    const filters = useComicFilters();
    const items = useLibraryStore((s) => s.groups)
        .map((g) => g.entries)
        .flat();
    const comicsType = useComicsTypeStore((s) => s.type);

    const writer = filters[COMIC_FILTERS.writer];

    return (
        <Layout>
            <section className={styles.page}>
                <header className={styles.header}>
                    <button
                        type="button"
                        className={styles.backButton}
                        onClick={() => navigate('/')}
                        aria-label="Back to library"
                    >
                        <ArrowLeftIcon size={16} />
                    </button>

                    <div className={styles.summary}>
                        <span className={styles.eyebrow}>Filtered by writer</span>
                        <h1 className={styles.title}>{writer || 'All comics'}</h1>
                    </div>

                    {writer ? (
                        <button
                            type="button"
                            className={styles.clearButton}
                            onClick={() => navigate('/filters')}
                        >
                            <CloseIcon size={12} />
                            Clear filter
                        </button>
                    ) : null}
                </header>

                {items.length === 0 ? (
                    <p className={styles.empty}>No comics in your library yet.</p>
                ) : (
                    <section
                        className={[
                            styles.comicContainer,
                            comicsType === 'detail' ? styles.detailLayout : '',
                        ]
                            .filter(Boolean)
                            .join(' ')}
                    >
                        <ItemsGrid key={writer ?? 'all'} items={items} filters={filters} />
                    </section>
                )}
            </section>
        </Layout>
    );
}
