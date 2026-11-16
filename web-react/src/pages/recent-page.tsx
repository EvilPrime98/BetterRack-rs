import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { ItemsGrid } from '@/components/items-grid/items-grid';
import { Layout } from '@/layout';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import { useRecentlyAdded } from '@/hooks/useRecentlyAdded';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import { RECENT_WINDOW_OPTIONS, type TRecentWindow } from '@/library.types';
import styles from './recent-page.module.css';
import { BRDropdown } from '@/components/br-dropdown/br-dropdown';
import { LayoutSelector } from '@/components/layout/layout-selector';
import { StateFilter } from '@/components/state-filter/state-filter';

const WINDOW_DROPDOWN_OPTIONS = RECENT_WINDOW_OPTIONS.map((opt) => ({
    value: opt.label,
    label: opt.label,
}));

export function RecentPage() {
    const [searchParams] = useSearchParams();

    const filter = searchParams.get('filter');

    const navigate = useNavigate();

    const [selectedWindow, setSelectedWindow] = useState<TRecentWindow>(
        () => RECENT_WINDOW_OPTIONS.find((opt) => opt.label === filter) || RECENT_WINDOW_OPTIONS[0],
    );

    const { items, isLoading, error } = useRecentlyAdded(selectedWindow.hours);

    const comicsType = useComicsTypeStore((s) => s.type);

    const onSelect = (label: TRecentWindow['label']) => {
        const option = RECENT_WINDOW_OPTIONS.find((opt) => opt.label === label);
        if (!option) return;
        setSelectedWindow(option);
        navigate(`/new?filter=${option.label}`);
    };

    return (
        <Layout>
            <section className={styles.page}>
                <header className={styles.header}>
                    <div className={styles.left}>
                        <button
                            type="button"
                            className={styles.backButton}
                            onClick={() => navigate('/')}
                            aria-label="Back to library"
                        >
                            <ArrowLeftIcon size={16} />
                        </button>

                        <div className={styles.summary}>
                            <span className={styles.eyebrow}>Recently added</span>
                            <h1 className={styles.title}>{selectedWindow.label}</h1>
                        </div>
                    </div>

                    <div className={styles.right}>
                        <StateFilter />
                        <LayoutSelector />
                        <BRDropdown
                            options={WINDOW_DROPDOWN_OPTIONS}
                            value={selectedWindow.label}
                            onChange={onSelect}
                            aria-label="Time window"
                        />
                    </div>
                </header>

                {isLoading ? (
                    <p className={styles.empty}>Loading recently added comics…</p>
                ) : error ? (
                    <p className={styles.empty}>{error}</p>
                ) : items.length === 0 ? (
                    <p className={styles.empty}>
                        Nothing added in the {selectedWindow.label.toLowerCase()}.
                    </p>
                ) : (
                    <section
                        className={[
                            styles.comicContainer,
                            comicsType === 'detail' ? styles.detailLayout : '',
                        ]
                            .filter(Boolean)
                            .join(' ')}
                    >
                        <ItemsGrid key={selectedWindow.label} items={items} />
                    </section>
                )}
            </section>
        </Layout>
    );
}
