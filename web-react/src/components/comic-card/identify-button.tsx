import type { CSSProperties } from 'react';
import { useComicIdentStore } from '@/stores/comicIdent.store';
import { CrButton } from '@/components/cr-button/cr-button';

export function IdentifyButton({ uid, style }: { uid: string; style?: CSSProperties }) {
    const onClick = () => {
        useComicIdentStore.getState().setItemUid(uid);
        useComicIdentStore.getState().setIsVisible(true);
    };

    return (
        <CrButton
            text="Identify"
            aria-label="Identify this comic"
            style={style}
            onClick={onClick}
        />
    );
}
