// Bridge between the Rust side and the two AppKit/MediaPlayer APIs that make a
// media app feel native on macOS:
//
//   * MPNowPlayingInfoCenter  -> the "Now Playing" tile in Control Center, the
//                                lock screen, the Dynamic Island on iPhone
//                                mirroring, AirPods announcements, etc.
//   * MPRemoteCommandCenter   -> hardware media keys (F7/F8/F9, Touch Bar,
//                                headphone buttons, Bluetooth remotes).
//
// These have no usable Rust bindings, and hand-writing objc2 `msg_send!` calls
// for blocks-with-return-values is far more error-prone than 150 lines of
// Objective-C. So we keep the FFI surface tiny and C-shaped.

#import <AppKit/AppKit.h>
#import <Foundation/Foundation.h>
#import <MediaPlayer/MediaPlayer.h>

// Keep in sync with `src/nowplaying.rs`.
typedef enum {
  YTM_CMD_PLAY = 1,
  YTM_CMD_PAUSE = 2,
  YTM_CMD_TOGGLE = 3,
  YTM_CMD_NEXT = 4,
  YTM_CMD_PREVIOUS = 5,
  YTM_CMD_SEEK = 6,
  YTM_CMD_STOP = 7,
  YTM_CMD_SKIP_FORWARD = 8,
  YTM_CMD_SKIP_BACKWARD = 9,
} YtmCommand;

typedef void (*YtmRemoteCallback)(int command, double value);

static YtmRemoteCallback g_callback = NULL;
static NSMutableDictionary *g_info = nil;
static NSMutableDictionary<NSString *, MPMediaItemArtwork *> *g_artCache = nil;
static NSString *g_artInFlight = nil;

static void ytm_emit(YtmCommand command, double value) {
  if (g_callback) {
    g_callback((int)command, value);
  }
}

static NSString *ytm_str(const char *s) {
  return s ? [NSString stringWithUTF8String:s] : @"";
}

// ---------------------------------------------------------------------------
// Remote commands (media keys)
// ---------------------------------------------------------------------------

void ytm_native_init(YtmRemoteCallback callback) {
  g_callback = callback;
  g_info = [NSMutableDictionary dictionary];
  g_artCache = [NSMutableDictionary dictionary];

  MPRemoteCommandCenter *center = [MPRemoteCommandCenter sharedCommandCenter];

  center.playCommand.enabled = YES;
  [center.playCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_PLAY, 0);
    return MPRemoteCommandHandlerStatusSuccess;
  }];

  center.pauseCommand.enabled = YES;
  [center.pauseCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_PAUSE, 0);
    return MPRemoteCommandHandlerStatusSuccess;
  }];

  center.togglePlayPauseCommand.enabled = YES;
  [center.togglePlayPauseCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_TOGGLE, 0);
    return MPRemoteCommandHandlerStatusSuccess;
  }];

  center.nextTrackCommand.enabled = YES;
  [center.nextTrackCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_NEXT, 0);
    return MPRemoteCommandHandlerStatusSuccess;
  }];

  center.previousTrackCommand.enabled = YES;
  [center.previousTrackCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_PREVIOUS, 0);
    return MPRemoteCommandHandlerStatusSuccess;
  }];

  center.stopCommand.enabled = YES;
  [center.stopCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_STOP, 0);
    return MPRemoteCommandHandlerStatusSuccess;
  }];

  // Lets the user scrub the Control Center progress bar.
  center.changePlaybackPositionCommand.enabled = YES;
  [center.changePlaybackPositionCommand
      addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
        MPChangePlaybackPositionCommandEvent *pos = (MPChangePlaybackPositionCommandEvent *)e;
        ytm_emit(YTM_CMD_SEEK, pos.positionTime);
        return MPRemoteCommandHandlerStatusSuccess;
      }];

  center.skipForwardCommand.preferredIntervals = @[ @10 ];
  center.skipForwardCommand.enabled = YES;
  [center.skipForwardCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_SKIP_FORWARD, 10);
    return MPRemoteCommandHandlerStatusSuccess;
  }];

  center.skipBackwardCommand.preferredIntervals = @[ @10 ];
  center.skipBackwardCommand.enabled = YES;
  [center.skipBackwardCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) {
    ytm_emit(YTM_CMD_SKIP_BACKWARD, 10);
    return MPRemoteCommandHandlerStatusSuccess;
  }];
}

// ---------------------------------------------------------------------------
// Now Playing info
// ---------------------------------------------------------------------------

static void ytm_fetch_artwork(NSString *url) {
  if (url.length == 0 || [url isEqualToString:g_artInFlight]) {
    return;
  }
  g_artInFlight = url;

  NSURL *parsed = [NSURL URLWithString:url];
  if (!parsed) {
    return;
  }

  NSURLSessionDataTask *task = [[NSURLSession sharedSession]
        dataTaskWithURL:parsed
      completionHandler:^(NSData *data, NSURLResponse *response, NSError *error) {
        if (!data || error) {
          return;
        }
        NSImage *image = [[NSImage alloc] initWithData:data];
        if (!image) {
          return;
        }
        MPMediaItemArtwork *artwork =
            [[MPMediaItemArtwork alloc] initWithBoundsSize:image.size
                                            requestHandler:^NSImage *(CGSize size) {
                                              return image;
                                            }];
        dispatch_async(dispatch_get_main_queue(), ^{
          if (g_artCache.count > 16) {
            [g_artCache removeAllObjects];
          }
          g_artCache[url] = artwork;
          g_info[MPMediaItemPropertyArtwork] = artwork;
          [MPNowPlayingInfoCenter defaultCenter].nowPlayingInfo = g_info;
        });
      }];
  [task resume];
}

void ytm_native_update(const char *title, const char *artist, const char *album,
                       const char *artworkUrl, double duration, double elapsed, int playing) {
  NSString *t = ytm_str(title);
  NSString *ar = ytm_str(artist);
  NSString *al = ytm_str(album);
  NSString *art = ytm_str(artworkUrl);

  dispatch_async(dispatch_get_main_queue(), ^{
    MPNowPlayingInfoCenter *center = [MPNowPlayingInfoCenter defaultCenter];

    g_info[MPMediaItemPropertyTitle] = t;
    g_info[MPMediaItemPropertyArtist] = ar;
    g_info[MPMediaItemPropertyAlbumTitle] = al;
    g_info[MPMediaItemPropertyPlaybackDuration] = @(duration);
    g_info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = @(elapsed);
    g_info[MPNowPlayingInfoPropertyPlaybackRate] = @(playing ? 1.0 : 0.0);
    g_info[MPNowPlayingInfoPropertyMediaType] = @(MPNowPlayingInfoMediaTypeAudio);

    if (art.length > 0) {
      MPMediaItemArtwork *cached = g_artCache[art];
      if (cached) {
        g_info[MPMediaItemPropertyArtwork] = cached;
      } else {
        ytm_fetch_artwork(art);
      }
    } else {
      [g_info removeObjectForKey:MPMediaItemPropertyArtwork];
    }

    center.nowPlayingInfo = g_info;
    center.playbackState =
        playing ? MPNowPlayingPlaybackStatePlaying : MPNowPlayingPlaybackStatePaused;
  });
}

void ytm_native_clear(void) {
  dispatch_async(dispatch_get_main_queue(), ^{
    [g_info removeAllObjects];
    MPNowPlayingInfoCenter *center = [MPNowPlayingInfoCenter defaultCenter];
    center.nowPlayingInfo = nil;
    center.playbackState = MPNowPlayingPlaybackStateStopped;
  });
}

// ---------------------------------------------------------------------------
// Activation
// ---------------------------------------------------------------------------

// Bring the app forward. tao can raise the window, but on macOS the
// application itself also has to be activated or the window comes up behind
// whatever is currently frontmost. / bring the app forward.
void ytm_native_activate(void) {
  dispatch_async(dispatch_get_main_queue(), ^{
    [NSApp activateIgnoringOtherApps:YES];
  });
}
