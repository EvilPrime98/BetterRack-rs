package dev.betterrack.android;

import android.app.Activity;
import android.app.Fragment;
import android.content.Intent;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;

/**
 * Opens the system folder picker from a NativeActivity, which has no onActivityResult hook of its
 * own: a headless framework Fragment receives the result instead. Loaded at runtime from the
 * embedded dex (see build-dex.sh); Rust registers {@link #onPicked}.
 */
public final class SafPicker extends Fragment {
    private static final int REQUEST = 7001;

    /** Called with the picked tree URI, or null when cancelled. */
    static native void onPicked(String uri);

    public static void launch(final Activity activity) {
        new Handler(Looper.getMainLooper()).post(new Runnable() {
            @Override
            public void run() {
                activity.getFragmentManager().beginTransaction().add(new SafPicker(), "saf-picker").commit();
            }
        });
    }

    @Override
    public void onCreate(android.os.Bundle state) {
        super.onCreate(state);
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        startActivityForResult(intent, REQUEST);
    }

    @Override
    public void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != REQUEST) return;
        String picked = null;
        if (result == Activity.RESULT_OK && data != null && data.getData() != null) {
            Uri uri = data.getData();
            getActivity().getContentResolver()
                .takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            picked = uri.toString();
        }
        getFragmentManager().beginTransaction().remove(this).commit();
        onPicked(picked);
    }
}
