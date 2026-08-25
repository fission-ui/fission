package rs.fission.runtime;

import androidx.games.activity.GameActivity;
import android.media.MediaPlayer;
import android.media.PlaybackParams;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.VideoView;

import java.util.HashMap;
import java.util.Map;

public final class FissionActivity extends GameActivity {
    private static volatile FissionActivity INSTANCE;
    private static final Map<Long, FissionVideoSlot> VIDEOS = new HashMap<>();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        INSTANCE = this;
    }

    @Override
    protected void onDestroy() {
        runOnUiThread(() -> {
            synchronized (VIDEOS) {
                for (FissionVideoSlot slot : VIDEOS.values()) {
                    slot.destroy();
                }
                VIDEOS.clear();
            }
        });
        INSTANCE = null;
        super.onDestroy();
    }

    public static void fissionCreateVideo(long id, String source) {
        runOnUiThreadOrRecordError(id, () -> {
            synchronized (VIDEOS) {
                FissionVideoSlot previous = VIDEOS.remove(id);
                if (previous != null) {
                    previous.destroy();
                }
                FissionVideoSlot slot = new FissionVideoSlot(INSTANCE, source);
                VIDEOS.put(id, slot);
            }
        });
    }

    public static void fissionUpdateVideoSurface(
            long id,
            int left,
            int top,
            int width,
            int height,
            boolean visible
    ) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.update(left, top, width, height, visible);
            }
        });
    }

    public static void fissionSetVideoVisible(long id, boolean visible) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null && slot.view != null) {
                slot.view.setVisibility(visible ? View.VISIBLE : View.GONE);
            }
        });
    }

    public static void fissionDestroyVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            synchronized (VIDEOS) {
                FissionVideoSlot slot = VIDEOS.remove(id);
                if (slot != null) {
                    slot.destroy();
                }
            }
        });
    }

    public static void fissionPlayVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.ended = false;
                slot.view.start();
            }
        });
    }

    public static void fissionPauseVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.view.pause();
            }
        });
    }

    public static void fissionStopVideo(long id) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.view.pause();
                slot.view.seekTo(0);
                slot.ended = false;
            }
        });
    }

    public static void fissionSeekVideo(long id, long positionMs) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.view.seekTo((int)Math.max(0L, Math.min(positionMs, Integer.MAX_VALUE)));
            }
        });
    }

    public static void fissionSetVideoRate(long id, float rate) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.rate = Math.max(0.1f, rate);
                slot.applyPlaybackParams();
            }
        });
    }

    public static void fissionSetVideoVolume(long id, float volume) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.volume = Math.max(0.0f, Math.min(volume, 1.0f));
                slot.applyVolume();
            }
        });
    }

    public static void fissionSetVideoMuted(long id, boolean muted) {
        runOnUiThreadOrRecordError(id, () -> {
            FissionVideoSlot slot = slot(id);
            if (slot != null) {
                slot.muted = muted;
                slot.applyVolume();
            }
        });
    }

    public static long fissionVideoPosition(long id) {
        FissionVideoSlot slot = slot(id);
        return slot == null || slot.view == null ? 0L : Math.max(0, slot.view.getCurrentPosition());
    }

    public static long fissionVideoDuration(long id) {
        FissionVideoSlot slot = slot(id);
        return slot == null || !slot.ready ? -1L : Math.max(0, slot.durationMs);
    }

    public static boolean fissionVideoReady(long id) {
        FissionVideoSlot slot = slot(id);
        return slot != null && slot.ready;
    }

    public static boolean fissionVideoEnded(long id) {
        FissionVideoSlot slot = slot(id);
        return slot != null && slot.ended;
    }

    public static String fissionVideoError(long id) {
        FissionVideoSlot slot = slot(id);
        return slot == null ? null : slot.error;
    }

    private static FissionVideoSlot slot(long id) {
        synchronized (VIDEOS) {
            return VIDEOS.get(id);
        }
    }

    private static void runOnUiThreadOrRecordError(long id, Runnable action) {
        FissionActivity activity = INSTANCE;
        if (activity == null) {
            recordError(id, "Fission Android video host is not attached to FissionActivity");
            return;
        }
        activity.runOnUiThread(() -> {
            try {
                action.run();
            } catch (Throwable error) {
                recordError(id, "Android video host error: " + error);
            }
        });
    }

    private static void recordError(long id, String error) {
        synchronized (VIDEOS) {
            FissionVideoSlot slot = VIDEOS.get(id);
            if (slot == null) {
                slot = new FissionVideoSlot(error);
                VIDEOS.put(id, slot);
            } else {
                slot.error = error;
            }
        }
    }

    private static final class FissionVideoSlot {
        final VideoView view;
        MediaPlayer mediaPlayer;
        volatile boolean ready;
        volatile boolean ended;
        volatile int durationMs = -1;
        volatile String error;
        volatile float rate = 1.0f;
        volatile float volume = 1.0f;
        volatile boolean muted;

        FissionVideoSlot(String error) {
            this.view = null;
            this.error = error;
        }

        FissionVideoSlot(FissionActivity activity, String source) {
            this.view = new VideoView(activity);
            this.view.setVisibility(View.GONE);
            this.view.setZOrderOnTop(true);
            this.view.setOnPreparedListener(player -> {
                mediaPlayer = player;
                ready = true;
                ended = false;
                durationMs = Math.max(0, view.getDuration());
                applyVolume();
                applyPlaybackParams();
            });
            this.view.setOnCompletionListener(player -> ended = true);
            this.view.setOnErrorListener((player, what, extra) -> {
                error = "Android MediaCodec playback error: what=" + what + ", extra=" + extra;
                return true;
            });
            this.view.setVideoPath(source);
            FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(1, 1);
            activity.addContentView(this.view, params);
        }

        void update(int left, int top, int width, int height, boolean visible) {
            if (view == null) {
                return;
            }
            FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
                    Math.max(1, width),
                    Math.max(1, height)
            );
            view.setLayoutParams(params);
            view.setX(left);
            view.setY(top);
            view.setVisibility(visible ? View.VISIBLE : View.GONE);
        }

        void applyPlaybackParams() {
            if (mediaPlayer == null) {
                return;
            }
            PlaybackParams params = mediaPlayer.getPlaybackParams();
            params.setSpeed(rate);
            mediaPlayer.setPlaybackParams(params);
        }

        void applyVolume() {
            if (mediaPlayer == null) {
                return;
            }
            float effective = muted ? 0.0f : volume;
            mediaPlayer.setVolume(effective, effective);
        }

        void destroy() {
            if (view == null) {
                return;
            }
            view.stopPlayback();
            ViewGroup parent = (ViewGroup)view.getParent();
            if (parent != null) {
                parent.removeView(view);
            }
        }
    }
}
