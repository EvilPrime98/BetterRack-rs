import { Link } from 'react-router-dom';
import styles from './folder-card.module.css';
import { ChevronDownIcon } from '@/icons/chevron.icon';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
//import { FolderCardStack } from "./folder-card-stack";
import { FolderCardBasic } from './folder-card-basic';
import { FolderCardActions } from './folder-card-actions';

export function FolderCard({ title, uid }: { title: string; uid: string }) {
    const comicsType = useComicsTypeStore((s) => s.type);

    return (
        <article
            className={[
                styles.folderCard,
                ...(comicsType === 'detail' ? [styles.detailMode] : []),
            ].join(' ')}
        >
            <Link to={`/${uid}`} className={styles.cardLink}>
                {/* {   (stackCovers.length)
                    ? <FolderCardStack
                        title={title}
                        stackCovers={stackCovers} 
                    />
                    : <FolderCardBasic 
                        title={title} 
                    />
                } */}

                <FolderCardBasic title={title} />

                <div className={styles.body}>
                    <span className={styles.kind}>Folder</span>
                    <p className={styles.title}>{title}</p>
                </div>

                <span className={styles.chevron} aria-hidden="true">
                    <ChevronDownIcon size={14} color="currentColor" />
                </span>

                <FolderCardActions title={title} uid={uid} />
            </Link>
        </article>
    );
}
