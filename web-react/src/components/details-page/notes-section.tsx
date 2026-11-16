import styles from '@/pages/details-page.module.css';
import { SectionBlock } from './section-block';

export function NotesSection({ notes }: { notes?: string[] }) {
    if (!notes?.length) return null;
    return (
        <SectionBlock title="Notes">
            <ul className={styles.plainList}>
                {notes.map((note) => (
                    <li key={note}>{note}</li>
                ))}
            </ul>
        </SectionBlock>
    );
}
