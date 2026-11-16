import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { Layout } from '@/layout';
import { useEffect } from 'react';
import styles from './details-page.module.css';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import { useComicById } from '@/hooks/useComicById';
import { ComicContent } from '@/components/details-page/content';

export function DetailsPage() {
    const { pageId } = useParams<{ pageId?: string }>();
    const [searchParams] = useSearchParams();
    const navigate = useNavigate();
    const sourceWiki = searchParams.get('sourceWiki');
    const { comic, loading, error, fetchComic } = useComicById();

    useEffect(() => {
        fetchComic(pageId, sourceWiki ?? undefined);
    }, [pageId, sourceWiki, fetchComic]);

    return (
        <Layout>
            <section className={styles.page}>
                <header className={styles.header}>
                    <button
                        type="button"
                        className={styles.backButton}
                        onClick={() => navigate(-1)}
                        aria-label="Go back"
                    >
                        <ArrowLeftIcon size={16} />
                    </button>

                    <div className={styles.summary}>
                        <span className={styles.eyebrow}>
                            {comic?.volume || 'Comic'}
                            {comic?.issue ? ` #${comic.issue}` : ''}
                        </span>
                        <h1 className={styles.title}>
                            {comic?.title || (loading ? 'Loading…' : 'Comic details')}
                        </h1>
                    </div>
                </header>

                {loading ? (
                    <div className={styles.stateMessage}>Loading comic details…</div>
                ) : error ? (
                    <div className={styles.stateMessage}>{error}</div>
                ) : comic ? (
                    <ComicContent comic={comic} navigate={navigate} />
                ) : null}
            </section>
        </Layout>
    );
}
