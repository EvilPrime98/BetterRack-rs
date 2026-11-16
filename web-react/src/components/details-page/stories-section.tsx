import styles from '@/pages/details-page.module.css';
import { SectionBlock } from './section-block';

export function StoriesSection({ storyTitles }: { storyTitles?: string[] }) {
    if (!storyTitles?.length) return null;
    return (
        <SectionBlock title="Stories">
            <ul className={styles.plainList}>
                {storyTitles.map((storyTitle) => (
                    <li key={storyTitle}>{storyTitle}</li>
                ))}
            </ul>
        </SectionBlock>
    );
}
