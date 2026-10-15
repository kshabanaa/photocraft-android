package local.photocraft.spike;

import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.ContentResolver;
import android.content.Intent;
import android.content.UriPermission;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import android.system.ErrnoException;
import android.system.Os;
import android.util.Log;
import android.view.KeyEvent;
import androidx.activity.OnBackPressedCallback;
import androidx.activity.result.ActivityResult;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import com.google.androidgamesdk.GameActivity;
import java.io.EOFException;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.SyncFailedException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicLong;

/** PhotoCraft's activity: GameActivity plus the platform behaviour the Rust side can't set itself. */
public class PhotoCraftActivity extends GameActivity {
    private static final String TAG = "PhotoCraft";

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
                Log.w(TAG, "TMPDIR not set", e);
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

    // ---------------------------------------------------------------------------------------------
    // Documents through the Storage Access Framework (WP 1.5/1.6; FR-FIO-05 to 08 and 11).
    //
    // The native side (apps/photocraft-android/src/bridge.rs) calls these methods from its own
    // threads. They only translate between Android and the native side: which document to open,
    // what to name a new one and what may be written are decided in Rust (documents.rs). Document
    // contents cross as bytes (a chunked read, a staged file to copy from), never as paths.
    // Methods that take a while (queries, reads, writes) run on the calling native thread, never
    // on the main thread; the pickers are started on the main thread.
    // ---------------------------------------------------------------------------------------------

    /** The picker in flight (the app shows one at a time): the native request id, or -1. */
    private int pickerRequest = -1;
    /** Picker answers waiting for the native side: {request, "ok", uri...}, {request, "cancel"} or {request, "error", message}. */
    private final ConcurrentLinkedQueue<String[]> pickerResults = new ConcurrentLinkedQueue<>();
    /** Documents being read, by handle (see openRead). */
    private final ConcurrentHashMap<Long, InputStream> reads = new ConcurrentHashMap<>();
    private final AtomicLong nextRead = new AtomicLong(1);

    // Registered as the activity is constructed, as the Activity Result API requires (before STARTED).
    private final ActivityResultLauncher<Intent> picker =
            registerForActivityResult(new ActivityResultContracts.StartActivityForResult(), this::onPicked);

    /**
     * Shows the system picker for documents to open (ACTION_OPEN_DOCUMENT). The answer arrives
     * through takePickerResult.
     *
     * @param mimeTypes the MIME types to offer, one per line ("*&#47;*" lists every file)
     * @param initialUri a document whose folder the picker starts in, or null
     */
    public void openDocuments(int request, String mimeTypes, boolean multiple, String initialUri) {
        runOnUiThread(() -> {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT)
                    .addCategory(Intent.CATEGORY_OPENABLE)
                    .setType("*/*")
                    .putExtra(Intent.EXTRA_ALLOW_MULTIPLE, multiple)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION
                            | Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                            | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            String[] types = mimeTypes.isEmpty() ? new String[0] : mimeTypes.split("\n");
            if (types.length > 0) {
                intent.putExtra(Intent.EXTRA_MIME_TYPES, types);
            }
            launchPicker(request, intent, initialUri);
        });
    }

    /**
     * Shows the system picker for where to create a document (ACTION_CREATE_DOCUMENT), offering
     * name as its name. The answer arrives through takePickerResult.
     */
    public void createDocument(int request, String mimeType, String name, String initialUri) {
        runOnUiThread(() -> {
            Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT)
                    .addCategory(Intent.CATEGORY_OPENABLE)
                    .setType(mimeType)
                    .putExtra(Intent.EXTRA_TITLE, name);
            launchPicker(request, intent, initialUri);
        });
    }

    private void launchPicker(int request, Intent intent, String initialUri) {
        if (initialUri != null && !initialUri.isEmpty()) {
            intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, Uri.parse(initialUri));
        }
        pickerRequest = request;
        try {
            picker.launch(intent);
        } catch (ActivityNotFoundException | IllegalStateException e) {
            pickerRequest = -1;
            pickerResults.add(new String[] {Integer.toString(request), "error", "No document picker on this device (" + e + ")"});
        }
    }

    private void onPicked(ActivityResult result) {
        int request = pickerRequest;
        pickerRequest = -1;
        if (request < 0) {
            // The process was restarted while the picker was showing: nothing is waiting for it.
            return;
        }
        Intent data = result.getData();
        if (result.getResultCode() != RESULT_OK || data == null) {
            pickerResults.add(new String[] {Integer.toString(request), "cancel"});
            return;
        }
        List<String> answer = new ArrayList<>();
        answer.add(Integer.toString(request));
        answer.add("ok");
        ClipData clip = data.getClipData();
        if (clip != null) {
            for (int i = 0; i < clip.getItemCount(); i++) {
                Uri uri = clip.getItemAt(i).getUri();
                if (uri != null) {
                    answer.add(keep(uri, data.getFlags()));
                }
            }
        } else if (data.getData() != null) {
            answer.add(keep(data.getData(), data.getFlags()));
        }
        pickerResults.add(answer.toArray(new String[0]));
    }

    /**
     * Keeps access to a chosen document across restarts (File › Open Recent, FR-FIO-11) when the
     * provider allows it; otherwise access lasts while PhotoCraft runs. The native side releases
     * grants that drop out of the recent list.
     */
    private String keep(Uri uri, int flags) {
        int mode = flags & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        if ((flags & Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION) != 0 && mode != 0) {
            try {
                getContentResolver().takePersistableUriPermission(uri, mode);
            } catch (SecurityException e) {
                Log.w(TAG, "no lasting access to a chosen document", e);
            }
        }
        return uri.toString();
    }

    /** The oldest picker answer not yet taken, or null. */
    public String[] takePickerResult() {
        return pickerResults.poll();
    }

    /**
     * What the native side needs to know about a document: {display name, MIME type, size in
     * bytes ("" when unknown), "1" when PhotoCraft may write to it, else "0"}.
     *
     * @throws SecurityException when access was withdrawn
     * @throws FileNotFoundException when the document is gone
     */
    public String[] describeDocument(String uriString) throws FileNotFoundException {
        Uri uri = Uri.parse(uriString);
        ContentResolver resolver = getContentResolver();
        boolean document = DocumentsContract.isDocumentUri(this, uri);
        String[] columns = document
                ? new String[] {OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE, DocumentsContract.Document.COLUMN_MIME_TYPE, DocumentsContract.Document.COLUMN_FLAGS}
                : new String[] {OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE};
        String name = null;
        String size = "";
        String mime = null;
        int flags = -1;
        try (Cursor c = resolver.query(uri, columns, null, null, null)) {
            if (c == null || !c.moveToFirst()) {
                throw new FileNotFoundException("no such document");
            }
            name = c.isNull(0) ? null : c.getString(0);
            size = c.isNull(1) ? "" : Long.toString(c.getLong(1));
            if (document) {
                mime = c.isNull(2) ? null : c.getString(2);
                flags = c.isNull(3) ? -1 : c.getInt(3);
            }
        }
        if (mime == null) {
            mime = resolver.getType(uri);
        }
        boolean granted = checkCallingOrSelfUriPermission(uri, Intent.FLAG_GRANT_WRITE_URI_PERMISSION) == PackageManager.PERMISSION_GRANTED;
        boolean writable = granted && (flags < 0 || (flags & DocumentsContract.Document.FLAG_SUPPORTS_WRITE) != 0);
        return new String[] {name == null ? "" : name, mime == null ? "" : mime, size, writable ? "1" : "0"};
    }

    /**
     * Opens a document for reading; returns a handle for readChunk and closeRead. The native side
     * reads in chunks into its own memory, so a large document never sits on the Java heap.
     */
    public long openRead(String uriString) throws FileNotFoundException {
        InputStream in = getContentResolver().openInputStream(Uri.parse(uriString));
        if (in == null) {
            throw new FileNotFoundException("the provider returned no stream");
        }
        long handle = nextRead.getAndIncrement();
        reads.put(handle, in);
        return handle;
    }

    /** Reads up to buffer.length bytes; returns how many, or -1 at the end. */
    public int readChunk(long handle, byte[] buffer) throws IOException {
        InputStream in = reads.get(handle);
        if (in == null) {
            throw new IOException("the document is not open for reading");
        }
        return in.read(buffer, 0, buffer.length);
    }

    public void closeRead(long handle) {
        InputStream in = reads.remove(handle);
        if (in != null) {
            try {
                in.close();
            } catch (IOException e) {
                Log.w(TAG, "closing a read document", e);
            }
        }
    }

    /**
     * Replaces the document's contents with the staged file's: the native side encodes the
     * document completely into a temporary file in app storage and syncs it first, so a failed
     * encode never touches the document. The target is then opened without truncation ("rw")
     * where the provider allows it, written from the start and cut to length only once every byte
     * is in; providers that only accept "wt" get that. Any failure closes the target with an
     * error, so providers that stage writes (cloud ones) drop the partial copy and keep the
     * original. The staged file stays with the native side, which keeps it until a save succeeds.
     *
     * @param stagedFd the staged file's descriptor, owned by the caller (it is duplicated here)
     * @param length how many bytes to copy from it
     */
    public void writeDocument(String uriString, int stagedFd, long length) throws IOException {
        Uri uri = Uri.parse(uriString);
        try (ParcelFileDescriptor staged = ParcelFileDescriptor.fromFd(stagedFd)) {
            FileChannel source = new FileInputStream(staged.getFileDescriptor()).getChannel();
            source.position(0);
            ParcelFileDescriptor target = openForReplace(uri);
            boolean closed = false;
            try {
                FileChannel sink = new FileOutputStream(target.getFileDescriptor()).getChannel();
                ByteBuffer buffer = ByteBuffer.allocate(1 << 20);
                long copied = 0;
                while (copied < length) {
                    buffer.clear();
                    long left = length - copied;
                    if (left < buffer.capacity()) {
                        buffer.limit((int) left);
                    }
                    int n = source.read(buffer);
                    if (n < 0) {
                        throw new EOFException("the staged copy is shorter than expected");
                    }
                    buffer.flip();
                    while (buffer.hasRemaining()) {
                        sink.write(buffer);
                    }
                    copied += n;
                }
                // A regular file opened "rw" keeps its old tail until it is cut; pipes have none.
                if (target.getStatSize() >= 0) {
                    sink.truncate(length);
                }
                try {
                    target.getFileDescriptor().sync();
                } catch (SyncFailedException e) {
                    // Pipes and sockets (some cloud providers) can't be synced; their provider
                    // learns the outcome from close() below.
                }
                closed = true;
                // A provider reports a failed upload here.
                target.close();
            } catch (IOException | RuntimeException e) {
                if (!closed) {
                    try {
                        target.closeWithError(e.toString());
                    } catch (IOException | RuntimeException ignored) {
                        // Already failing: the first error is the one to report.
                    }
                }
                throw e;
            }
        }
    }

    /** The document opened for writing: "rw" (no truncation) when the provider takes it, else "wt". */
    private ParcelFileDescriptor openForReplace(Uri uri) throws FileNotFoundException {
        ContentResolver resolver = getContentResolver();
        try {
            ParcelFileDescriptor fd = resolver.openFileDescriptor(uri, "rw");
            if (fd != null) {
                return fd;
            }
        } catch (FileNotFoundException | SecurityException | IllegalArgumentException | UnsupportedOperationException e) {
            // This provider writes only in "w" modes (or not at all: "wt" reports why).
        }
        ParcelFileDescriptor fd = resolver.openFileDescriptor(uri, "wt");
        if (fd == null) {
            throw new FileNotFoundException("the provider returned no file to write");
        }
        return fd;
    }

    /** The lasting grants PhotoCraft holds: {uri, mode, uri, mode, ...} with modes "r", "w" or "rw". */
    public String[] persistedGrants() {
        List<UriPermission> held = getContentResolver().getPersistedUriPermissions();
        String[] out = new String[held.size() * 2];
        for (int i = 0; i < held.size(); i++) {
            UriPermission p = held.get(i);
            out[2 * i] = p.getUri().toString();
            out[2 * i + 1] = (p.isReadPermission() ? "r" : "") + (p.isWritePermission() ? "w" : "");
        }
        return out;
    }

    /** Gives up the lasting grant for a document that left the recent list. */
    public void releaseGrant(String uriString) {
        Uri uri = Uri.parse(uriString);
        ContentResolver resolver = getContentResolver();
        for (UriPermission p : resolver.getPersistedUriPermissions()) {
            if (p.getUri().equals(uri)) {
                int mode = (p.isReadPermission() ? Intent.FLAG_GRANT_READ_URI_PERMISSION : 0)
                        | (p.isWritePermission() ? Intent.FLAG_GRANT_WRITE_URI_PERMISSION : 0);
                resolver.releasePersistableUriPermission(uri, mode);
            }
        }
    }

    /** Where PhotoCraft keeps files that must survive a crash but never go to a backup. */
    public String noBackupDir() {
        return getNoBackupFilesDir().getAbsolutePath();
    }
}
