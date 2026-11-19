import type { ReleaseDate, WikiComic } from 'better-wiki';
import styles from './comic-card.module.css';
import type { TMetaSource } from '@/library.types';
import { Link } from 'react-router-dom';
import { InfoRow } from './info-row';
import { writerHref } from '@/lib/writer-filter';
import { Loader } from '@/components/loader/loader';
import { FandomIcon } from '@/icons/fandom.icon';
import { XmlIcon } from '@/icons/xml.icon';
//import { CrButton } from '../cr-button/cr-button';

const metaSourceLabel: Record<TMetaSource, string> = {
    wiki: 'From wiki',
    comicinfo: 'From ComicInfo.xml',
};

function formatReleaseDate(releaseDate?: ReleaseDate | null): string {
    const { releaseMonth, releaseDay, releaseYear } = releaseDate || {};
    if (!releaseMonth || !releaseDay || !releaseYear) return '';
    return [releaseMonth, releaseDay].map((n) => n.padStart(2, '0')).join('/') + `/${releaseYear}`;
}

export function ComicCardInfo({
    comic,
    metaSource,
    isLoading,
}: {
    comic: WikiComic | null;
    metaSource?: TMetaSource;
    isLoading?: boolean;
}) {
    if (isLoading) {
        return (
            <div className={styles.info}>
                <Loader visible size={16} className={styles.infoLoader} />
            </div>
        );
    }

    if (!comic) {
        return (
            <div className={styles.info}>
                <p className={styles.infoEmpty}>No information found.</p>
            </div>
        );
    }

    const released = formatReleaseDate(comic?.releaseDate);

    const writers = comic?.credits?.writers || [];

    //const viewMoreURL = `/details/${comic?.pageId}?sourceWiki=${comic?.sourceWiki}`;

    return (
        <div className={styles.info}>
            {metaSource && (
                <div
                    className={styles.metaSourceBadge}
                    role="img"
                    aria-label={metaSourceLabel[metaSource]}
                    title={metaSourceLabel[metaSource]}
                >
                    {metaSource === 'wiki' ? (
                        <FandomIcon size={11} />
                    ) : (
                        <XmlIcon size={15} color="#334155" />
                    )}
                </div>
            )}

            <InfoRow label="Comic" value={comic?.title || ''} />

            <InfoRow label="Volume" value={comic?.volume || ''} />

            <InfoRow label="Issue" value={comic?.issue || ''} />

            <InfoRow label="Year" value={comic?.releaseDate?.releaseYear || ''} />

            <InfoRow label="Writer" value={writers.join(', ')}>
                {writers.map((writer, i) => (
                    <span key={writer}>
                        {i > 0 && ', '}
                        <Link to={writerHref(writer)} onClick={(e) => e.stopPropagation()}>
                            {writer}
                        </Link>
                    </span>
                ))}
            </InfoRow>

            <InfoRow label="Artist" value={(comic?.credits?.artists || []).join(', ')} />

            <InfoRow label="Released" value={released} />

            {/* <CrButton
                text="View More"
                className={styles.viewMore}
                onClick={() => navigate(viewMoreURL)}
            /> */}
        </div>
    );
}
