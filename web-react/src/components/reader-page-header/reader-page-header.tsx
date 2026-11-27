import { useEffect, useRef, useState } from 'react';
import type { FocusEvent, KeyboardEvent } from 'react';
import styles from '../../pages/reader.page.module.css';
import { ArrowLeftIcon } from '../../icons/arrow-left.icon';
import { BookmarkIcon } from '../../icons/bookmark.icon';
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
    const [menuOpen, setMenuOpen] = useState(false);
    const menuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!menuOpen) return;
        function onPointerDown(event: PointerEvent) {
            if (!menuRef.current?.contains(event.target as Node)) setMenuOpen(false);
        }
        document.addEventListener('pointerdown', onPointerDown);
        return () => document.removeEventListener('pointerdown', onPointerDown);
    }, [menuOpen]);

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
            <button
                type="button"
                className={styles.toolButton}
                aria-label="Library"
                onClick={goBack}
            >
                <ArrowLeftIcon size={16} />
                <span>Library</span>
            </button>

            {bookmarks.length > 0 && (
                <div ref={menuRef} className={styles.bookmarkWrap}>
                    <button
                        type="button"
                        className={styles.toolButton}
                        aria-label="Bookmarks"
                        aria-expanded={menuOpen}
                        onClick={() => setMenuOpen((open) => !open)}
                    >
                        <BookmarkIcon size={16} />
                        <span>Bookmarks</span>
                    </button>

                    {menuOpen && (
                        <div className={styles.bookmarkMenu}>
                            {bookmarks.map((bookmark) => (
                                <button
                                    key={bookmark.page}
                                    type="button"
                                    className={styles.bookmarkItem}
                                    onClick={() => {
                                        goToPage(bookmark.page);
                                        setMenuOpen(false);
                                    }}
                                >
                                    {`${bookmark.label} · p.${bookmark.page}`}
                                </button>
                            ))}
                        </div>
                    )}
                </div>
            )}

            <h1 className={styles.title} title={title}>
                {title}
            </h1>

            <BRDropdown<TReadingMode>
                className={styles.readingMode}
                fit
                alignRight
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
                    type="text"
                    inputMode="numeric"
                    pattern="[0-9]*"
                    spellCheck={false}
                    aria-label="Current page"
                    defaultValue={currentPage}
                    onFocus={(event) => event.currentTarget.select()}
                    onKeyDown={onCounterKeyDown}
                    onBlur={onCounterBlur}
                />
                {`/ ${totalPages}`}
            </span>

            <button
                type="button"
                className={[styles.toolButton, isRefreshing ? styles.spinning : '']
                    .filter(Boolean)
                    .join(' ')}
                aria-label="Refresh scan"
                onClick={onRefresh}
            >
                <RefreshIcon size={16} />
            </button>
        </header>
    );
}
