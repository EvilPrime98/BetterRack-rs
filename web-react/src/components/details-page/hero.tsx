import type { WikiComic } from 'better-wiki';
import styles from '@/pages/details-page.module.css';
import { ImageGen } from '@/components/image-generic/image-generic';
import { MetaItem } from './meta-item';
import { CreditsList } from './credits-list';
import type { CreditRow, Navigate } from './details-page.types';

export function ComicHero({
    comic,
    released,
    creditRows,
    firstWriter,
    navigate,
}: {
    comic: WikiComic;
    released: string;
    creditRows: CreditRow[];
    firstWriter?: string;
    navigate: Navigate;
}) {
    return (
        <div className={styles.hero}>
            <div className={styles.coverWrap}>
                <ImageGen className={styles.cover} src={comic.cover} alt={comic.title} />
                {comic.event ? <span className={styles.eventTag}>{comic.event}</span> : null}
            </div>

            <div className={styles.heroInfo}>
                <div className={styles.metaGrid}>
                    {comic.volume ? <MetaItem label="Volume" value={comic.volume} /> : null}
                    {comic.issue ? <MetaItem label="Issue" value={`#${comic.issue}`} /> : null}
                    {released ? <MetaItem label="Released" value={released} /> : null}
                    {comic.rating ? <MetaItem label="Rating" value={comic.rating} /> : null}
                </div>

                <CreditsList
                    creditRows={creditRows}
                    firstWriter={firstWriter}
                    navigate={navigate}
                />

                {comic.quotation?.quote ? (
                    <blockquote className={styles.quotation}>
                        <p>&ldquo;{comic.quotation.quote}&rdquo;</p>
                        {comic.quotation.speaker ? <cite>{comic.quotation.speaker}</cite> : null}
                    </blockquote>
                ) : null}
            </div>
        </div>
    );
}
