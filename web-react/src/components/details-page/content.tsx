import type { WikiComic } from 'better-wiki';
import styles from '@/pages/details-page.module.css';
import { ComicHero } from './hero';
import { SectionBlock } from './section-block';
import { StoriesSection } from './stories-section';
import { AppearingSection } from './appearing-section';
import { CoverVariantsSection } from './cover-variants-section';
import { NotesSection } from './notes-section';
import { TriviaSection } from './trivia-section';
import { formatReleaseDate, getCreditRows, getAppearingGroups } from './details-page.utils';
import type { Navigate } from './details-page.types';

export function ComicContent({ comic, navigate }: { comic: WikiComic; navigate: Navigate }) {
    const released = formatReleaseDate(comic.releaseDate);
    const creditRows = getCreditRows(comic.credits);
    const firstWriter = comic.credits?.writers?.[0];
    const appearingGroups = getAppearingGroups(comic.appearing);

    return (
        <div className={styles.content}>
            <ComicHero
                comic={comic}
                released={released}
                creditRows={creditRows}
                firstWriter={firstWriter}
                navigate={navigate}
            />

            {comic.synopsis ? (
                <SectionBlock title="Synopsis">
                    <p className={styles.synopsis}>{comic.synopsis}</p>
                </SectionBlock>
            ) : null}

            <StoriesSection storyTitles={comic.storyTitles} />

            <AppearingSection groups={appearingGroups} />

            <CoverVariantsSection variants={comic.coverVariants} />

            <NotesSection notes={comic.notes} />

            <TriviaSection trivia={comic.trivia} />
        </div>
    );
}
