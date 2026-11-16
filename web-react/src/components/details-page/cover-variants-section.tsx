import type { WikiComicCoverVariant } from 'better-wiki';
import styles from '@/pages/details-page.module.css';
import { ImageGen } from '@/components/image-generic/image-generic';
import { SectionBlock } from './section-block';

export function CoverVariantsSection({ variants }: { variants?: WikiComicCoverVariant[] }) {
    if (!variants?.length) return null;
    return (
        <SectionBlock title="Cover Variants">
            <div className={styles.variantGrid}>
                {variants.map((variant) => (
                    <div key={variant.coverNumber} className={styles.variantCard}>
                        {variant.imageUrl ? (
                            <ImageGen
                                className={styles.variantImage}
                                src={variant.imageUrl}
                                alt={variant.imageLabel || `Cover ${variant.coverNumber}`}
                            />
                        ) : null}
                        <span className={styles.variantLabel}>
                            {variant.imageLabel || `Cover ${variant.coverNumber}`}
                        </span>
                        {variant.artists.length > 0 ? (
                            <span className={styles.variantArtists}>
                                {variant.artists.join(', ')}
                            </span>
                        ) : null}
                    </div>
                ))}
            </div>
        </SectionBlock>
    );
}
