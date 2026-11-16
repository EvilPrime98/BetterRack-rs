import type { WikiAppearanceEntry } from 'better-wiki';
import styles from '@/pages/details-page.module.css';
import { SectionBlock } from './section-block';
import type { AppearingGroup } from './details-page.types';

export function AppearingSection({ groups }: { groups: AppearingGroup[] }) {
    if (groups.length === 0) return null;
    return (
        <SectionBlock title="Appearing">
            <div className={styles.appearingGroups}>
                {groups.map((group) => (
                    <div key={group.label} className={styles.appearingGroup}>
                        <span className={styles.appearingGroupLabel}>{group.label}</span>
                        <div className={styles.chipList}>
                            {group.entries.map((entry: WikiAppearanceEntry) => (
                                <span
                                    key={entry.name}
                                    className={styles.chip}
                                    title={entry.statusNote || undefined}
                                >
                                    {entry.name}
                                </span>
                            ))}
                        </div>
                    </div>
                ))}
            </div>
        </SectionBlock>
    );
}
