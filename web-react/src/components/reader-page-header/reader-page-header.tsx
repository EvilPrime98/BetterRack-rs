import type { FocusEvent, KeyboardEvent } from 'react';
import styles from '../../pages/reader.page.module.css';
import { ArrowLeftIcon } from '../../icons/arrow-left.icon';
import { RefreshIcon } from '../../icons/refresh-icon';
import type { IBookmark } from '../../library.types';
import type { TReadingMode } from '../../stores/userPref.store';
import { BRDropdown } from '../br-dropdown/br-dropdown';

export function ReaderPageHeader({
    title,
    currentPage,
    totalPages,
    bookmarks,
    isRefreshing,
    readingMode,
    onReadingModeChange,
    goToPage,
    goBack,
    onRefresh,
}: {
    title: string;
    currentPage: number;
    totalPages: number;
    bookmarks: IBookmark[];
    isRefreshing: boolean;
    readingMode: TReadingMode;
    onReadingModeChange: (mode: TReadingMode) => void;
    goToPage: (page: number) => void;
    goBack: () => void;
    onRefresh: () => void;
}) {
    const onBookmarkSelect = (value: string) => {
        const page = Number(value);
        if (Number.isInteger(page) && page >= 1) goToPage(page);
    };

    const onCounterKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
        if (event.key === 'Enter') {
            event.currentTarget.blur();
        } else if (event.key === 'Escape') {
            event.currentTarget.value = String(currentPage);
            event.currentTarget.blur();
        }
    };

    const onCounterBlur = (event: FocusEvent<HTMLInputElement>) => {
        const input = event.currentTarget;
        const page = Number(input.value.trim());
        if (Number.isInteger(page) && page >= 1) {
            const target = Math.min(page, totalPages);
            if (target !== currentPage) goToPage(target);
        }
        input.value = String(currentPage);
    };

    return (
        <header className={styles.toolbar}>
            <div className={styles.toolbarStart}>
                <button type="button" className={styles.back} onClick={goBack}>
                    <ArrowLeftIcon size={16} />
                    <span>Library</span>
                </button>
                {bookmarks.length > 0 && (
                    <BRDropdown
                        className={styles.bookmarks}
                        aria-label="Jump to bookmark"
                        triggerLabel="Jump to bookmark…"
                        value={String(currentPage)}
                        options={bookmarks.map((bookmark) => ({
                            value: String(bookmark.page),
                            label: `${bookmark.label} · p.${bookmark.page}`,
                        }))}
                        onChange={onBookmarkSelect}
                    />
                )}
            </div>
            <h1 className={styles.title} title={title}>
                {title}
            </h1>
            <div className={styles.toolbarEnd}>
                <BRDropdown<TReadingMode>
                    className={styles.bookmarks}
                    aria-label="Reading mode"
                    value={readingMode}
                    options={[
                        { value: 'vertical', label: 'Vertical' },
                        { value: 'horizontal', label: 'Horizontal' },
                    ]}
                    onChange={onReadingModeChange}
                />
                <span className={styles.counter}>
                    <input
                        key={currentPage}
                        className={styles.counterInput}
                        style={{ width: `${String(totalPages).length + 3}ch` }}
                        type="text"
                        inputMode="numeric"
                        pattern="[0-9]*"
                        spellCheck={false}
                        aria-label="Current page"
                        defaultValue={currentPage}
                        onFocus={(event) => event.currentTarget.select()}
                        onKeyDown={onCounterKeyDown}
                        onBlur={onCounterBlur}
                    />{' '}
                    / {totalPages}
                </span>
                <button
                    type="button"
                    className={[styles.refresh, isRefreshing ? styles.spinning : '']
                        .filter(Boolean)
                        .join(' ')}
                    aria-label="Refresh scan"
                    onClick={onRefresh}
                >
                    <RefreshIcon size={16} />
                </button>
            </div>
        </header>
    );
}
