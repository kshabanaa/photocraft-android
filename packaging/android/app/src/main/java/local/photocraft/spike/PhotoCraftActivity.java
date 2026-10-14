package local.photocraft.spike;

import android.os.Bundle;
import android.system.ErrnoException;
import android.system.Os;
import android.util.Log;
import android.view.KeyEvent;
import androidx.activity.OnBackPressedCallback;
import com.google.androidgamesdk.GameActivity;
import java.io.File;

/** PhotoCraft's activity: GameActivity plus the platform behaviour the Rust side can't set itself. */
public class PhotoCraftActivity extends GameActivity {
    /**
     * The modifier keys (KeyEvent meta state) held at the last press of another key, and held now.
     * winit 0.30 never reports modifiers on Android, so the native side reads these to give
     * Ctrl+Z, Shift+drag and the other shortcuts their modifiers.
     */
    public volatile int keyMeta;
    public volatile int heldMeta;

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        heldMeta = event.getMetaState();
        if (event.getAction() == KeyEvent.ACTION_DOWN && !KeyEvent.isModifierKey(event.getKeyCode())) {
            keyMeta = event.getMetaState();
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Rust's std::env::temp_dir() falls back to /data/local/tmp, which apps can't write; set it
        // before the native library loads (Rust 2024 makes set_var unsafe on its side).
        File tmp = new File(getCacheDir(), "tmp");
        if (tmp.isDirectory() || tmp.mkdirs()) {
            try {
                Os.setenv("TMPDIR", tmp.getAbsolutePath(), true);
            } catch (ErrnoException e) {
                Log.w("PhotoCraft", "TMPDIR not set", e);
            }
        }
        super.onCreate(savedInstanceState);
        // Back never closes PhotoCraft and its open documents: it sends the app to the background,
        // as Home does (targetSdk 36 delivers Back here, not as a key event).
        getOnBackPressedDispatcher().addCallback(this, new OnBackPressedCallback(true) {
            @Override
            public void handleOnBackPressed() {
                moveTaskToBack(true);
            }
        });
    }

    /**
     * The soft keyboard's action key (Enter on the number pad, Done, Go) arrives as an editor
     * action, which winit 0.30 drops. Hand it on as an Enter key press, so a number field commits
     * the way a hardware Enter makes it.
     */
    @Override
    public void onEditorAction(int action) {
        super.onEditorAction(action);
        long now = android.os.SystemClock.uptimeMillis();
        onKeyDown(KeyEvent.KEYCODE_ENTER, new KeyEvent(now, now, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER, 0));
        onKeyUp(KeyEvent.KEYCODE_ENTER, new KeyEvent(now, now, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER, 0));
    }
}
