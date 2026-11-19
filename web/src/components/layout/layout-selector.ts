import { CycleButton } from '@/components/cycle-button/cycle-button';
import { COMICS_TYPE_CTX } from '@/context/comics-types.context';

export function LayoutSelector() {
    return CycleButton({
        state: COMICS_TYPE_CTX.type,
        onNext: COMICS_TYPE_CTX.next,
    });
}
