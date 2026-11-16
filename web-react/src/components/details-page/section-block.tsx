import type { ReactNode } from 'react';
import styles from '@/pages/details-page.module.css';

export function SectionBlock({ title, children }: { title: string; children: ReactNode }) {
    return (
        <section className={styles.section}>
            <h2 className={styles.sectionTitle}>{title}</h2>
            {children}
        </section>
    );
}
