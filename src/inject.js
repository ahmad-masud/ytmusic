// Injected into every page the webview loads (main frame only).
//
// This is the part of the app that knows about YouTube Music specifically:
// it reads the player state out of the page and exposes a tiny command API
// that the Rust side calls with `evaluate_script`.
//
// Everything here is defensive. YouTube Music is a Polymer app whose internals
// change without notice, so each lookup has fallbacks and every failure is
// swallowed rather than allowed to break the page.

(function () {
  "use strict";

  if (window.__ytmNativeInstalled || window.top !== window) return;
  window.__ytmNativeInstalled = true;

  var POLL_MS = 1000;

  function post(type, data) {
    try {
      window.ipc.postMessage(JSON.stringify({ type: type, data: data }));
    } catch (e) {
      /* no host attached (e.g. page opened in a plain browser) */
    }
  }

  function warn(message) {
    post("log", String(message));
  }

  // --- element lookup ------------------------------------------------------
  // Fast path is a plain querySelector. Only if that misses do we walk shadow
  // roots, and results are cached until the node leaves the document.

  var cache = Object.create(null);
  // Walking shadow roots visits ~10k nodes on YouTube Music. Remember which
  // lookups came up empty so a selector that matches nothing (the like button
  // before you sign in, say) does not pay that cost on every tick.
  var missedAt = Object.create(null);
  var DEEP_RETRY_MS = 10000;

  function deepQuery(selector, root) {
    var hit = root.querySelector(selector);
    if (hit) return hit;
    var all = root.querySelectorAll("*");
    for (var i = 0; i < all.length; i++) {
      if (all[i].shadowRoot) {
        var nested = deepQuery(selector, all[i].shadowRoot);
        if (nested) return nested;
      }
    }
    return null;
  }

  function find(key, selectors) {
    var cached = cache[key];
    if (cached && cached.isConnected) return cached;

    // Cheap path, always taken: a plain selector over the light DOM, which is
    // where YouTube Music actually keeps the player bar.
    for (var i = 0; i < selectors.length; i++) {
      var hit = document.querySelector(selectors[i]);
      if (hit) {
        cache[key] = hit;
        missedAt[key] = 0;
        return hit;
      }
    }

    // Expensive path: only worth it if they ever move to real shadow roots.
    var now = Date.now();
    if (missedAt[key] && now - missedAt[key] < DEEP_RETRY_MS) {
      cache[key] = null;
      return null;
    }
    for (var j = 0; j < selectors.length; j++) {
      var deep = deepQuery(selectors[j], document);
      if (deep) {
        cache[key] = deep;
        missedAt[key] = 0;
        return deep;
      }
    }
    missedAt[key] = now;
    cache[key] = null;
    return null;
  }

  function video() {
    return find("video", ["video.html5-main-video", "#movie_player video", "video"]);
  }

  function playPauseButton() {
    return find("playPause", [
      "ytmusic-player-bar #play-pause-button",
      "#play-pause-button",
      "ytmusic-player-bar tp-yt-paper-icon-button.play-pause-button",
    ]);
  }

  function nextButton() {
    return find("next", [
      "ytmusic-player-bar .next-button",
      ".next-button",
      "ytmusic-player-bar tp-yt-paper-icon-button[title*='Next']",
    ]);
  }

  function prevButton() {
    return find("prev", [
      "ytmusic-player-bar .previous-button",
      ".previous-button",
      "ytmusic-player-bar tp-yt-paper-icon-button[title*='Previous']",
    ]);
  }

  function likeRenderer() {
    return find("likeRenderer", [
      "ytmusic-player-bar ytmusic-like-button-renderer",
      "#main-panel ytmusic-like-button-renderer",
    ]);
  }

  function likeButton() {
    return find("like", [
      "ytmusic-player-bar #button-shape-like button",
      "ytmusic-player-bar ytmusic-like-button-renderer #button-shape-like button",
      "ytmusic-player-bar tp-yt-paper-icon-button.like",
    ]);
  }

  function dislikeButton() {
    return find("dislike", [
      "ytmusic-player-bar #button-shape-dislike button",
      "ytmusic-player-bar ytmusic-like-button-renderer #button-shape-dislike button",
      "ytmusic-player-bar tp-yt-paper-icon-button.dislike",
    ]);
  }

  function click(node) {
    if (!node) return false;
    node.click();
    return true;
  }

  function isPlaying() {
    var v = video();
    return !!(v && !v.paused && !v.ended && v.readyState > 2);
  }

  function toggle() {
    if (!click(playPauseButton())) {
      var v = video();
      if (!v) return;
      if (v.paused) v.play();
      else v.pause();
    }
  }

  // --- command API --------------------------------------------------------

  var commands = {
    play: function () {
      if (!isPlaying()) toggle();
    },
    pause: function () {
      if (isPlaying()) toggle();
    },
    playPause: toggle,
    stop: function () {
      var v = video();
      if (isPlaying()) toggle();
      if (v) v.currentTime = 0;
    },
    next: function () {
      click(nextButton());
    },
    previous: function () {
      click(prevButton());
    },
    seekTo: function (seconds) {
      var v = video();
      if (v && isFinite(seconds)) v.currentTime = Math.max(0, seconds);
    },
    seekBy: function (delta) {
      var v = video();
      if (!v) return;
      var limit = isFinite(v.duration) ? v.duration : Infinity;
      v.currentTime = Math.max(0, Math.min(limit, v.currentTime + delta));
    },
    volumeBy: function (delta) {
      var v = video();
      if (!v) return;
      v.volume = Math.max(0, Math.min(1, v.volume + delta));
      if (v.volume > 0) v.muted = false;
    },
    toggleMute: function () {
      var v = video();
      if (v) v.muted = !v.muted;
    },
    like: function () {
      click(likeButton());
    },
    dislike: function () {
      click(dislikeButton());
    },
    shuffle: function () {
      click(find("shuffle", ["ytmusic-player-bar .shuffle", ".shuffle.ytmusic-player-bar"]));
    },
    repeat: function () {
      click(find("repeat", ["ytmusic-player-bar .repeat", ".repeat.ytmusic-player-bar"]));
    },
    search: function () {
      var box = find("search", ["ytmusic-search-box input", "input#input"]);
      if (box) {
        box.focus();
        box.select();
      }
    },
    copyLink: function () {
      post("copyLink", trackUrl());
    },
  };

  window.__ytmNative = {
    cmd: function (name, arg) {
      try {
        var fn = commands[name];
        if (!fn) {
          warn("unknown command: " + name);
          return;
        }
        fn(arg);
        // Report the result of the command promptly rather than waiting for
        // the next poll, so the menu bar feels instant.
        setTimeout(function () {
          publish(true);
        }, 120);
      } catch (e) {
        warn("command " + name + " failed: " + e);
      }
    },
    refresh: function () {
      publish(true);
    },
  };

  // --- state scraping -----------------------------------------------------

  function trackUrl() {
    try {
      var id = new URLSearchParams(location.search).get("v");
      if (id) return "https://music.youtube.com/watch?v=" + id;
      var link = find("shareLink", ["ytmusic-player-bar a#endpoint"]);
      if (link && link.href) return link.href;
    } catch (e) {
      /* fall through */
    }
    return location.href;
  }

  // An <img> with an empty src resolves to the document URL, and the player bar
  // has one of those before a track loads. Only accept things that really look
  // like cover art.
  function usableArtwork(src) {
    if (!src || /^data:/i.test(src)) return "";
    try {
      var parsed = new URL(src, location.href);
      if (parsed.href === location.href) return "";
      if (/(googleusercontent\.com|ytimg\.com|ggpht\.com)$/i.test(parsed.hostname)) return src;
      if (/\.(jpe?g|png|webp|gif)$/i.test(parsed.pathname)) return src;
    } catch (e) {
      /* not a URL we can reason about */
    }
    return "";
  }

  // YouTube serves thumbnails at the size baked into the URL; ask for one big
  // enough for the Control Center tile.
  function upgradeArtwork(url) {
    var src = usableArtwork(url);
    if (!src) return "";
    return src
      .replace(/=w\d+-h\d+/, "=w600-h600")
      .replace(/\/(?:default|mqdefault|hqdefault)\.jpg/, "/maxresdefault.jpg");
  }

  function fromMediaSession() {
    try {
      var md = navigator.mediaSession && navigator.mediaSession.metadata;
      if (!md || !md.title) return null;
      var art = "";
      var images = md.artwork || [];
      var widest = -1;
      for (var i = 0; i < images.length; i++) {
        var size = parseInt((images[i].sizes || "0x0").split("x")[0], 10) || 0;
        if (size >= widest) {
          widest = size;
          art = images[i].src || "";
        }
      }
      return {
        title: md.title || "",
        artist: md.artist || "",
        album: md.album || "",
        artwork: upgradeArtwork(art),
      };
    } catch (e) {
      return null;
    }
  }

  function text(node) {
    return node ? (node.textContent || "").trim() : "";
  }

  function fromDom() {
    var title = text(find("barTitle", ["ytmusic-player-bar .title.ytmusic-player-bar"]));
    if (!title) {
      return { title: "", artist: "", album: "", artwork: "" };
    }
    var byline = text(find("barByline", ["ytmusic-player-bar .byline.ytmusic-player-bar"]));
    // The byline is "Artist • Album • Year"; keep the first two segments.
    var parts = byline.split("•").map(function (s) {
      return s.trim();
    });
    var image = find("barImage", [
      "ytmusic-player-bar #song-image img",
      "ytmusic-player-bar img.image",
    ]);
    return {
      title: title,
      artist: parts[0] || "",
      album: parts.length > 2 ? parts[1] : "",
      artwork: upgradeArtwork(image ? image.src : ""),
    };
  }

  function likeStatus() {
    var renderer = likeRenderer();
    var status = renderer ? renderer.getAttribute("like-status") : null;
    if (status === "LIKE") return 1;
    if (status === "DISLIKE") return -1;
    return 0;
  }

  function snapshot() {
    var v = video();
    var meta = fromMediaSession() || fromDom();
    return {
      title: meta.title,
      artist: meta.artist,
      album: meta.album,
      artwork: meta.artwork,
      duration: v && isFinite(v.duration) ? v.duration : 0,
      elapsed: v && isFinite(v.currentTime) ? v.currentTime : 0,
      playing: isPlaying(),
      liked: likeStatus(),
      hasPlayer: !!v,
      url: trackUrl(),
    };
  }

  var lastSerialised = "";

  function publish(force) {
    try {
      var state = snapshot();
      var serialised = JSON.stringify(state);
      if (!force && serialised === lastSerialised) return;
      lastSerialised = serialised;
      post("state", state);
    } catch (e) {
      warn("publish failed: " + e);
    }
  }

  // Media events do not bubble, so listen in the capture phase on `document`:
  // the capture pass still walks through `document` on its way to the target.
  ["play", "pause", "seeked", "ended", "loadedmetadata", "ratechange", "volumechange"].forEach(
    function (name) {
      document.addEventListener(
        name,
        function () {
          setTimeout(function () {
            publish(true);
          }, 50);
        },
        true
      );
    }
  );

  setInterval(publish, POLL_MS);
  publish(true);
  post("ready", null);
})();
