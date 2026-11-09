import { CycleButton } from '@/components/cycle-button/cycle-button';
import { READ_TYPES_CTX } from '@/context/read-types.context';

export function StateFilter() {
    return CycleButton({
        state: READ_TYPES_CTX.type,
        onNext: READ_TYPES_CTX.next,
    });
}
