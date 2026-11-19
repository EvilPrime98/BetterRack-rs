import { useCallback, useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import { toggleFullscreen } from '@/services/fullscreen.service';
import { setImmersiveMode } from '@/services/immersive.service';
import styles from './reader.page.module.css';
import { RENDER_AHEAD, RENDER_BEHIND } from '../utils/reader.page.utils';
import { ImageElement } from '@/components/reader-page-image/reader-page-image';
import { ReaderPageHeader } from '@/components/reader-page-header/reader-page-header';
import { ReaderPageProgressBar } from '@/components/reader-page-progress-bar/reader-page-progress-bar';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { ReaderNext } from '@/components/reader-next/reader-next';
import { useReaderNavigation } from '@/hooks/useReaderNavigation';
import { useComicPages } from '@/hooks/useComicPages';
import { useReaderPageTracking } from '@/hooks/useReaderPageTracking';
import { useReaderProgress } from '@/hooks/useReaderProgress';
import { useReaderZoom } from '@/hooks/useReaderZoom';
import { BRButton } from '@/components/br-button/br-button';
import { useReaderKeys } from '@/hooks/useReaderKeys';
import { useSettledValue } from '@/hooks/useSettledValue';

const ACTIVE_PAGE_SETTLE_MS = 150;

export function ReaderPage() {
    //hooks
    const { uid } = useParams<{ uid: string }>();
    const setTitle = useDocumentTitleStore((s) => s.setTitle);
    const { navigate, goBack } = useReaderNavigation();
    const { pages, title, isLoading, hasError, bookmarks, isRefreshing, loadPages, refreshComic } =
        useComicPages(uid);

    //refs
    const viewerRef = useRef<HTMLButtonElement>(null);
    const pageRef = useRef<HTMLElement>(null);

    const { currentPage, isReady, goToPage } = useReaderPageTracking(uid, pages, viewerRef);
    const activePage = useSettledValue(isReady ? currentPage : null, ACTIVE_PAGE_SETTLE_MS);

    const onPageRatio = useCallback((ratio: number) => {
        const $viewer = viewerRef.current;
        if ($viewer && !$viewer.style.getPropertyValue('--page-ratio')) {
            $viewer.style.setProperty('--page-ratio', String(ratio));
        }
    }, []);
    const { onWheel } = useReaderZoom(pageRef, viewerRef);
    useReaderProgress(uid, pages, currentPage);

    useReaderKeys({ pageRef });

    const [isHeaderVisible, setIsHeaderVisible] = useState(true);

    const toggleHeader = useCallback(() => {
        setIsHeaderVisible((visible) => !visible);
    }, []);

    useEffect(() => {
        setImmersiveMode(true);
        return () => setImmersiveMode(false);
    }, []);

    useEffect(() => {
        setTitle('Reader');
    }, [setTitle]);

    if (!uid) return null;

    const numPages = pages.length;
    const next = numPages > 0 && currentPage === numPages;

    return (
        <section
            className={styles.page}
            onWheel={onWheel}
            ref={pageRef}
        >
            <div className={`${styles.headerOverlay} ${isHeaderVisible ? '' : styles.hidden}`}>
                <ReaderPageHeader
                    title={title}
                    currentPage={currentPage}
                    totalPages={pages.length}
                    bookmarks={bookmarks}
                    isRefreshing={isRefreshing}
                    goToPage={goToPage}
                    goBack={goBack}
                    onRefresh={refreshComic}
                />
                <ReaderPageProgressBar currentPage={currentPage} totalPages={pages.length} />
            </div>

            <div className={styles.state} style={{ display: isLoading ? undefined : 'none' }}>
                <p>Loading pages…</p>
            </div>

            <div className={styles.state} style={{ display: hasError ? undefined : 'none' }}>
                <p>Something went wrong while loading this comic.</p>
                <BRButton text="Retry" onClick={loadPages} />
            </div>

            <button
                className={styles.viewer}
                ref={viewerRef}
                type="button"
                aria-label="Toggle reader header"
                onClick={toggleHeader}
                onDoubleClick={toggleFullscreen}
            >
                {Array.from({ length: Math.max(0, numPages - 1) }, (_, i) => i + 1).map((i) => (
                    <ImageElement
                        key={i}
                        uid={uid}
                        ind={i}
                        index={i + 1}
                        total={numPages}
                        active={
                            activePage !== null &&
                            i + 1 >= activePage - RENDER_BEHIND &&
                            i + 1 <= activePage + RENDER_AHEAD
                        }
                        onRatio={onPageRatio}
                    />
                ))}
            </button>

            {next ? <ReaderNext uid={uid} navigate={navigate} /> : null}
        </section>
    );
}
