# Privacy

* **Local only.** All data lives in one SQLite file in the app's data folder
  (Windows: `%APPDATA%\org.opendiag.app\opendiag.db`; Android: the app's private storage).
* **No account, no telemetry, no automatic upload.** Not the VIN, not reports, not location.
* **Update check.** The installed app asks GitHub's public API which OpenDiag
  release is the newest when it starts (`update.rs` in the app shell). The
  request contains nothing about the user or vehicle (GitHub sees the IP address,
  as with any web request). Settings → Updates turns it off. Updates are only
  downloaded when the user presses the button.
  Community features (submitting compatibility results) work by exporting a
  file the user chooses to share. Any future online service will be opt-in.
* **Sharing results (opt-in).** After a scan or hardware test the app can
  offer to share an anonymous result with the OpenDiag data service
  (`services/feedback`, a Cloudflare Worker with a D1 database). The app asks
  once; until the user says yes nothing is offered, and every share shows the
  exact data and needs a press on Send. A share holds: make, model and year,
  the manufacturer code (first 3 VIN characters, never the full VIN), adapter,
  connection type, protocol, calibration IDs, which functions worked, fault
  codes with freeze-frame values, and readiness monitors
  (`crates/opendiag-engine/src/share.rs`). No names, notes, location, files or
  full VIN. Simulator results are never shared. The service stores the share,
  the day it arrived and a hash of a delete token; it does not store IP
  addresses (a salted daily hash for rate limiting is deleted the next day).
  Settings → Sharing deletes one or all of a user's shares. Only combinations
  reported by at least two shares are published, as summaries.
* **Logs.** The adapter log hides VIN answers unless the user turns that off.
* **Exports** show what they contain before saving and mask the VIN unless the
  user ticks "Include the full VIN".
* **Deletion.** Settings → Privacy deletes every vehicle, session, report, log
  and clear snapshot. Deleting a vehicle in My Garage deletes its history.
* **At-rest encryption** is not implemented yet. Until it is, rely on device
  encryption (BitLocker, Android's default file-based encryption). Planned:
  encrypt the database with a key kept in the OS keystore (Windows DPAPI /
  Android Keystore).
