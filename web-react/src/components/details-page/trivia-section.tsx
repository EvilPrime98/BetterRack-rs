import styles from '@/pages/details-page.module.css';
import { SectionBlock } from './section-block';

export function TriviaSection({ trivia }: { trivia?: string[] }) {
    if (!trivia?.length) return null;
    return (
        <SectionBlock title="Trivia">
            <ul className={styles.plainList}>
                {trivia.map((fact) => (
                    <li key={fact}>{fact}</li>
                ))}
            </ul>
        </SectionBlock>
    );
}
