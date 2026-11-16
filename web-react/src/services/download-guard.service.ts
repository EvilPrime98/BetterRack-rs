import { createDownloadTracker } from '../lib/active-downloads';
import { installLeaveGuard } from '../lib/leave-guard';
import { getDownloadJobs } from './store.service';

export const DOWNLOAD_TRACKER = createDownloadTracker(getDownloadJobs);

export function registerLeaveGuard(): () => void {
    return installLeaveGuard(window, DOWNLOAD_TRACKER.count);
}
