import { COMIC_IDENT_CTX } from '../../context/identifer-modal.context';
import { CrButton } from '../cr-button/cr-button';

export function IdentifyButton({ uid }: { uid: string }) {
    const onClick = () => {
        COMIC_IDENT_CTX.itemUid.set(uid);
        COMIC_IDENT_CTX.isVisible.set(true);
    };

    return CrButton({
        text: 'Identify',
        attributes: {
            'aria-label': 'Identify this comic',
        },
        eventHandler: {
            click: onClick,
        },
    });
}
