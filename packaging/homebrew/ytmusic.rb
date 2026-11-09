# Homebrew cask for YouTube Music.
#
# This cannot go into the official homebrew/cask tap. Three separate blockers:
#
#   1. Acceptable Casks requires artefacts that "must pass Homebrew's Gatekeeper
#      checks and must not require System Integrity Protection or Gatekeeper to
#      be disabled or bypassed". This app is ad-hoc signed, so it fails on a
#      default macOS configuration. Clearing it needs an Apple Developer ID
#      certificate ($99/year) and notarisation.
#   2. Notability: a self-submission needs 90 forks, 90 watchers or 225 stars.
#   3. The repository must be at least 30 days old.
#
# It works fine from a personal tap, which the policy explicitly allows:
# "Software that does not meet the official criteria can generally be
# maintained in a third-party tap."
#
# To publish one:
#
#   gh repo create homebrew-tap --public -d "Homebrew tap"
#   git clone https://github.com/<you>/homebrew-tap && cd homebrew-tap
#   mkdir -p Casks && cp path/to/ytmusic.rb Casks/
#   git add -A && git commit -m "Add ytmusic cask" && git push
#
# Then: brew install --cask <you>/tap/ytmusic
#
# Note that Homebrew 7 removed --no-quarantine, so a cask install is still
# quarantined and Gatekeeper still prompts on first launch. See the caveats
# below. Building from source avoids it entirely.

cask "ytmusic" do
  version "0.1.0"
  sha256 "080b1b8a1aa74a214a4ddd492c747bc23fb2a1ae9e7108f6f2a5a92c743d5b83"

  url "https://github.com/ahmad-masud/ytmusic/releases/download/v#{version}/YouTube-Music-#{version}-universal.zip"
  name "YouTube Music"
  desc "Native desktop app for YouTube Music with media keys and Now Playing"
  homepage "https://github.com/ahmad-masud/ytmusic"

  depends_on macos: ">= :big_sur"

  app "YouTube Music.app"

  zap trash: [
    "~/Library/Application Support/YouTube Music",
    "~/Library/Caches/com.ahmadmasud.ytmusic",
    "~/Library/HTTPStorages/com.ahmadmasud.ytmusic",
    "~/Library/WebKit/com.ahmadmasud.ytmusic",
  ]

  caveats <<~EOS
    This build is ad-hoc signed rather than notarised, so macOS will refuse to
    open it the first time. To allow it:

      Open the app once, then go to
      System Settings -> Privacy & Security -> Open Anyway

    Or clear the quarantine attribute directly:

      xattr -dr com.apple.quarantine "#{appdir}/YouTube Music.app"

    Building from source skips this entirely, since a locally compiled binary
    is never quarantined.
  EOS
end
